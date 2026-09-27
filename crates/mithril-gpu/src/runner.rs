//! Host side: compile generated CUDA via docker nvcc (cached by source
//! hash) and run the .cubin through the CUDA driver API with the wave loop
//! ported from the bitonic spike: read per-rule bucket counters, launch the
//! biggest undrained bucket, repeat until quiescent, checking the device
//! abort flag every wave so arena exhaustion becomes a clean error.

use mithril_rt::Redex;
use std::ffi::c_void;
use std::fs;
use std::path::Path;
use std::process::Command;

// ---- hand-written CUDA driver API FFI (no external crates) ----

type CUresult = i32;
type CUdeviceptr = u64;

#[allow(non_snake_case)]
extern "C" {
    fn cuInit(flags: u32) -> CUresult;
    fn cuDeviceGet(device: *mut i32, ordinal: i32) -> CUresult;
    fn cuCtxCreate_v2(ctx: *mut *mut c_void, flags: u32, dev: i32) -> CUresult;
    fn cuCtxDestroy_v2(ctx: *mut c_void) -> CUresult;
    fn cuCtxSetLimit(limit: i32, value: usize) -> CUresult;
    fn cuCtxSynchronize() -> CUresult;
    fn cuModuleLoadData(module: *mut *mut c_void, image: *const c_void) -> CUresult;
    fn cuModuleGetFunction(f: *mut *mut c_void, module: *mut c_void, name: *const std::ffi::c_char) -> CUresult;
    fn cuModuleGetGlobal_v2(
        dptr: *mut CUdeviceptr,
        bytes: *mut usize,
        module: *mut c_void,
        name: *const std::ffi::c_char,
    ) -> CUresult;
    fn cuMemAlloc_v2(dptr: *mut CUdeviceptr, bytesize: usize) -> CUresult;
    fn cuMemGetInfo_v2(free: *mut usize, total: *mut usize) -> CUresult;
    fn cuDeviceGetAttribute(pi: *mut i32, attrib: i32, dev: i32) -> CUresult;
    fn cuMemsetD8_v2(dst: CUdeviceptr, uc: u8, n: usize) -> CUresult;
    fn cuMemcpyHtoD_v2(dst: CUdeviceptr, src: *const c_void, n: usize) -> CUresult;
    fn cuMemcpyDtoH_v2(dst: *mut c_void, src: CUdeviceptr, n: usize) -> CUresult;
    #[allow(clippy::too_many_arguments)]
    fn cuLaunchKernel(
        f: *mut c_void,
        gx: u32,
        gy: u32,
        gz: u32,
        bx: u32,
        by: u32,
        bz: u32,
        shared: u32,
        stream: *mut c_void,
        params: *mut *mut c_void,
        extra: *mut *mut c_void,
    ) -> CUresult;
}

const CU_LIMIT_STACK_SIZE: i32 = 0;
const CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT: i32 = 16;
const CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_MULTIPROCESSOR: i32 = 39;

fn cu(r: CUresult, what: &str) -> Result<(), String> {
    if r == 0 {
        Ok(())
    } else {
        Err(format!("mithril-gpu: CUDA driver call {what} failed with code {r}"))
    }
}

// ---- device-side Dev struct mirror (field order must match engine.cu) ----

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Dev {
    nodes: CUdeviceptr,
    recs: CUdeviceptr,
    nbump: CUdeviceptr,
    rbump: CUdeviceptr,
    nfree: CUdeviceptr,
    nfreen: CUdeviceptr,
    nchunk: CUdeviceptr,
    ovf: CUdeviceptr,
    ovftop: CUdeviceptr,
    ebuf: CUdeviceptr,
    blen: CUdeviceptr,
    bdone: CUdeviceptr,
    result: CUdeviceptr,
    abortf: CUdeviceptr,
    ncap: u32,
    rcap: u32,
    bcap: u32,
    ovfcap: u32,
    chunksz: u32,
    nrules: u32,
}

const REC_SIZE: usize = 40; // sizeof(Rec) in engine.cu
const MAXLANES: usize = 1 << 16;
const FREECAP: usize = 64;
const TPB: u32 = 256;

/// Parse a capacity env var: decimal, `0x..`, or `1<<k`.
fn env_cap(name: &str, default: u64) -> u64 {
    let Ok(s) = std::env::var(name) else { return default };
    let s = s.trim().replace(' ', "");
    let v = if let Some(hex) = s.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok()
    } else if let Some((a, b)) = s.split_once("<<") {
        match (a.parse::<u64>(), b.parse::<u32>()) {
            (Ok(a), Ok(b)) => a.checked_shl(b),
            _ => None,
        }
    } else {
        s.parse::<u64>().ok()
    };
    v.filter(|&v| v >= 2).unwrap_or(default)
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub(crate) const ENGINE_CU: &str = include_str!("../cuda/engine.cu");

/// The docker nvcc build command for `dir` (mounted at /w), per the global
/// constraint: the host has no nvcc.
fn nvcc_compile(dir: &Path) -> Result<(), String> {
    let dir = dir
        .canonicalize()
        .map_err(|e| format!("mithril-gpu: cache dir {}: {e}", dir.display()))?;
    let out = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--gpus",
            "all",
            "-v",
            &format!("{}:/w", dir.display()),
            "blaze-ptx:cu13x",
            "nvcc",
            "-O3",
            "-arch=sm_89",
            "-cubin",
            "/w/program.cu",
            "-o",
            "/w/program.cubin",
        ])
        .output()
        .map_err(|e| format!("mithril-gpu: failed to invoke docker: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "mithril-gpu: nvcc failed ({}):\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

/// Write `cu_src` + the fixed engine into a hash-keyed subdir of
/// `cache_dir`, compile to a .cubin via docker nvcc (skipped on a cache
/// hit), load it through the driver API and run the wave loop. Returns the
/// result port raw delivered to ROOT.
pub fn compile_and_run(cu_src: &str, boot: Redex, cache_dir: &Path) -> Result<u64, String> {
    // key on program AND engine source, so an engine change invalidates too
    let key = fnv1a(cu_src) ^ fnv1a(ENGINE_CU).rotate_left(1);
    let dir = cache_dir.join(format!("{key:016x}"));
    fs::create_dir_all(&dir).map_err(|e| format!("mithril-gpu: mkdir {}: {e}", dir.display()))?;
    let cubin_path = dir.join("program.cubin");
    if !cubin_path.exists() {
        fs::write(dir.join("program.cu"), cu_src)
            .map_err(|e| format!("mithril-gpu: write program.cu: {e}"))?;
        fs::write(dir.join("engine.cu"), ENGINE_CU)
            .map_err(|e| format!("mithril-gpu: write engine.cu: {e}"))?;
        nvcc_compile(&dir)?;
    }
    let cubin = fs::read(&cubin_path).map_err(|e| format!("mithril-gpu: read cubin: {e}"))?;
    GpuRunner::run(&cubin, boot)
}

/// Driver-API runner: loads a compiled .cubin and drives the wave loop.
pub struct GpuRunner;

impl GpuRunner {
    pub fn run(cubin: &[u8], boot: Redex) -> Result<u64, String> {
        unsafe {
            cu(cuInit(0), "cuInit")?;
            let mut dev = 0i32;
            cu(cuDeviceGet(&mut dev, 0), "cuDeviceGet")?;
            let mut ctx: *mut c_void = std::ptr::null_mut();
            cu(cuCtxCreate_v2(&mut ctx, 0, dev), "cuCtxCreate")?;
            let r = run_in_ctx(cubin, boot, dev);
            // Destroying the context releases every allocation made in it.
            let _ = cuCtxDestroy_v2(ctx);
            r
        }
    }
}

unsafe fn alloc(n: usize, what: &str) -> Result<CUdeviceptr, String> {
    let mut p: CUdeviceptr = 0;
    cu(cuMemAlloc_v2(&mut p, n), what)?;
    Ok(p)
}

unsafe fn dtoh<T: Copy + Default>(src: CUdeviceptr, n: usize, what: &str) -> Result<Vec<T>, String> {
    let mut v = vec![T::default(); n];
    cu(
        cuMemcpyDtoH_v2(v.as_mut_ptr() as *mut c_void, src, n * std::mem::size_of::<T>()),
        what,
    )?;
    Ok(v)
}

unsafe fn run_in_ctx(cubin: &[u8], boot: Redex, dev: i32) -> Result<u64, String> {
    let stack = env_cap("MITHRIL_GPU_STACK", 32 * 1024) as usize;
    cu(cuCtxSetLimit(CU_LIMIT_STACK_SIZE, stack), "cuCtxSetLimit(stack)")?;

    let mut module: *mut c_void = std::ptr::null_mut();
    cu(cuModuleLoadData(&mut module, cubin.as_ptr() as *const c_void), "cuModuleLoadData")?;

    // number of rules, published by the generated program
    let mut nr_ptr: CUdeviceptr = 0;
    let mut nr_sz = 0usize;
    cu(
        cuModuleGetGlobal_v2(&mut nr_ptr, &mut nr_sz, module, c"g_nrules".as_ptr()),
        "cuModuleGetGlobal(g_nrules)",
    )?;
    let nrules = dtoh::<u32>(nr_ptr, 1, "read g_nrules")?[0] as usize;
    if nrules == 0 || nrules > 1 << 16 {
        return Err(format!("mithril-gpu: implausible rule count {nrules}"));
    }

    // capacities. Buckets recycle once fully drained (see the wave loop), so
    // per-rule capacity only has to cover in-flight entries, and the default
    // scales a fixed total entry budget over the rule count rather than
    // growing ebuf linearly in nrules.
    let ncap_req = env_cap("MITHRIL_GPU_NODES", 1 << 28);
    let rcap = env_cap("MITHRIL_GPU_RECS", 1 << 24) as u32;
    let bcap = if std::env::var("MITHRIL_GPU_BUCKET").is_ok() {
        env_cap("MITHRIL_GPU_BUCKET", 1 << 20) as u32
    } else {
        ((1u64 << 24) / nrules as u64).clamp(1 << 14, 1 << 20) as u32
    };
    let ovfcap: u32 = 1 << 20;

    // Cap the cell arena to what this device can actually serve: free VRAM
    // minus the fixed buffers, the driver's local-memory (stack) reserve for
    // all resident threads, and slack for the context/module.
    let (mut vfree, mut vtotal) = (0usize, 0usize);
    cu(cuMemGetInfo_v2(&mut vfree, &mut vtotal), "cuMemGetInfo")?;
    let (mut mp, mut tpm) = (0i32, 0i32);
    cu(cuDeviceGetAttribute(&mut mp, CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT, dev), "attr(mp)")?;
    cu(
        cuDeviceGetAttribute(&mut tpm, CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_MULTIPROCESSOR, dev),
        "attr(threads/mp)",
    )?;
    let stack_reserve = (mp.max(1) as u64) * (tpm.max(1) as u64) * stack as u64;
    let fixed = REC_SIZE as u64 * rcap as u64
        + 24 * bcap as u64 * nrules as u64
        + (4 * MAXLANES * FREECAP + 12 * MAXLANES) as u64
        + 4 * ovfcap as u64
        + 8 * nrules as u64
        + (1 << 20);
    let slack: u64 = 1 << 30;
    let budget = (vfree as u64).saturating_sub(fixed + stack_reserve + slack);
    let max_ncap = (budget / 16).max(1 << 10).min(u32::MAX as u64 - 1);
    let ncap = if ncap_req > max_ncap {
        eprintln!(
            "mithril-gpu: warning: MITHRIL_GPU_NODES={ncap_req} does not fit in free VRAM \
             ({} MiB); capping the cell arena to {max_ncap} cells",
            vfree >> 20
        );
        max_ncap as u32
    } else {
        ncap_req as u32
    };
    let chunksz = ((ncap as u64) / 65536).clamp(2, 4096) as u32;

    // device buffers
    let d = Dev {
        nodes: alloc(16 * ncap as usize, "alloc nodes")?,
        recs: alloc(REC_SIZE * rcap as usize, "alloc recs")?,
        nbump: alloc(4, "alloc nbump")?,
        rbump: alloc(4, "alloc rbump")?,
        nfree: alloc(4 * MAXLANES * FREECAP, "alloc nfree")?,
        nfreen: alloc(4 * MAXLANES, "alloc nfreen")?,
        nchunk: alloc(8 * MAXLANES, "alloc nchunk")?,
        ovf: alloc(4 * ovfcap as usize, "alloc ovf")?,
        ovftop: alloc(4, "alloc ovftop")?,
        ebuf: alloc(24 * bcap as usize * nrules, "alloc ebuf")?,
        blen: alloc(4 * nrules, "alloc blen")?,
        bdone: alloc(4 * nrules, "alloc bdone")?,
        result: alloc(16, "alloc result")?,
        abortf: alloc(4, "alloc abortf")?,
        ncap,
        rcap,
        bcap,
        ovfcap,
        chunksz,
        nrules: nrules as u32,
    };
    cu(cuMemsetD8_v2(d.nodes, 0, 16), "memset cell0")?;
    cu(cuMemsetD8_v2(d.nfreen, 0, 4 * MAXLANES), "memset nfreen")?;
    cu(cuMemsetD8_v2(d.nchunk, 0, 8 * MAXLANES), "memset nchunk")?;
    cu(cuMemsetD8_v2(d.ovf, 0, 4 * ovfcap as usize), "memset ovf")?;
    cu(cuMemsetD8_v2(d.ovftop, 0, 4), "memset ovftop")?;
    cu(cuMemsetD8_v2(d.blen, 0, 4 * nrules), "memset blen")?;
    cu(cuMemsetD8_v2(d.bdone, 0, 4 * nrules), "memset bdone")?;
    cu(cuMemsetD8_v2(d.result, 0, 16), "memset result")?;
    cu(cuMemsetD8_v2(d.abortf, 0, 4), "memset abortf")?;
    let one: u32 = 1; // cell 0 and record 0 are reserved
    cu(cuMemcpyHtoD_v2(d.nbump, (&one as *const u32).cast(), 4), "init nbump")?;
    cu(cuMemcpyHtoD_v2(d.rbump, (&one as *const u32).cast(), 4), "init rbump")?;

    // publish Dev to the module global G
    let mut g_ptr: CUdeviceptr = 0;
    let mut g_sz = 0usize;
    cu(cuModuleGetGlobal_v2(&mut g_ptr, &mut g_sz, module, c"G".as_ptr()), "cuModuleGetGlobal(G)")?;
    if g_sz != std::mem::size_of::<Dev>() {
        return Err(format!(
            "mithril-gpu: Dev layout mismatch (device {} bytes, host {})",
            g_sz,
            std::mem::size_of::<Dev>()
        ));
    }
    cu(cuMemcpyHtoD_v2(g_ptr, (&d as *const Dev).cast(), g_sz), "upload G")?;

    let mut k_boot: *mut c_void = std::ptr::null_mut();
    cu(cuModuleGetFunction(&mut k_boot, module, c"k_boot".as_ptr()), "get k_boot")?;
    let mut k_fire: *mut c_void = std::ptr::null_mut();
    cu(cuModuleGetFunction(&mut k_fire, module, c"k_fire".as_ptr()), "get k_fire")?;
    let mut k_pump: *mut c_void = std::ptr::null_mut();
    cu(cuModuleGetFunction(&mut k_pump, module, c"k_pump".as_ptr()), "get k_pump")?;

    // boot fires rule 0 with the redex, parent = ROOT (aux)
    {
        let (mut a, mut b, mut c) = (boot.a, boot.b, boot.aux);
        let mut params = [
            (&mut a as *mut u64).cast::<c_void>(),
            (&mut b as *mut u64).cast::<c_void>(),
            (&mut c as *mut u64).cast::<c_void>(),
        ];
        cu(
            cuLaunchKernel(k_boot, 1, 1, 1, 1, 1, 1, 0, std::ptr::null_mut(), params.as_mut_ptr(), std::ptr::null_mut()),
            "launch k_boot",
        )?;
    }

    // Host wave loop. Bucket counters live on the device (`blen` append,
    // `bdone` drained prefix): each wave the host syncs, reads them back,
    // recycles fully drained buckets to slot 0 (so ebuf only has to hold a
    // wave's live entries, not the whole run's appends), then either fires
    // the largest bucket in parallel or — when the frontier is tiny, as in
    // a strict dependence chain — launches the device-side pump, which
    // batches up to PUMP_STEPS dependent rewrites in one kernel instead of
    // one host round-trip each.
    const PUMP_MAX_PENDING: u64 = 128;
    const PUMP_STEPS: u32 = 1 << 16;
    let wave_limit = env_cap("MITHRIL_GPU_WAVES", 1 << 24);
    let mut waves: u64 = 0;
    loop {
        cu(cuCtxSynchronize(), "cuCtxSynchronize")?;
        let ab = dtoh::<u32>(d.abortf, 1, "read abortf")?[0];
        if ab >= 2 {
            return Err(
                "mithril-gpu: arena exhausted (raise MITHRIL_GPU_NODES / MITHRIL_GPU_RECS / MITHRIL_GPU_BUCKET)"
                    .to_string(),
            );
        }
        if ab == 1 {
            return Err("mithril-gpu: unreachable match arm reached".to_string());
        }
        let mut lens = dtoh::<u32>(d.blen, nrules, "read blen")?;
        let mut dones = dtoh::<u32>(d.bdone, nrules, "read bdone")?;
        let mut best = usize::MAX;
        let mut best_n = 0u32;
        let mut total: u64 = 0;
        let mut recycled = false;
        for k in 0..nrules {
            let pend = lens[k].min(bcap).saturating_sub(dones[k]);
            if pend == 0 && lens[k] > 0 && dones[k] >= lens[k] {
                lens[k] = 0; // fully drained: recycle the bucket
                dones[k] = 0;
                recycled = true;
            }
            total += pend as u64;
            if pend > best_n {
                best_n = pend;
                best = k;
            }
        }
        if best == usize::MAX {
            break;
        }
        waves += 1;
        if waves > wave_limit {
            return Err(
                "mithril-gpu: wave limit exceeded (non-terminating program? raise MITHRIL_GPU_WAVES)"
                    .to_string(),
            );
        }
        if total <= PUMP_MAX_PENDING {
            // sequential tail: drain it on-device
            if recycled {
                cu(
                    cuMemcpyHtoD_v2(d.blen, lens.as_ptr().cast(), 4 * nrules),
                    "upload blen",
                )?;
            }
            cu(cuMemcpyHtoD_v2(d.bdone, dones.as_ptr().cast(), 4 * nrules), "upload bdone")?;
            let mut steps = PUMP_STEPS;
            let mut params = [(&mut steps as *mut u32).cast::<c_void>()];
            cu(
                cuLaunchKernel(k_pump, 1, 1, 1, 1, 1, 1, 0, std::ptr::null_mut(), params.as_mut_ptr(), std::ptr::null_mut()),
                "launch k_pump",
            )?;
            continue;
        }
        let (mut rule, mut start, mut count) = (best as u32, dones[best], best_n);
        dones[best] += best_n;
        if recycled {
            cu(cuMemcpyHtoD_v2(d.blen, lens.as_ptr().cast(), 4 * nrules), "upload blen")?;
        }
        cu(cuMemcpyHtoD_v2(d.bdone, dones.as_ptr().cast(), 4 * nrules), "upload bdone")?;
        let mut params = [
            (&mut rule as *mut u32).cast::<c_void>(),
            (&mut start as *mut u32).cast::<c_void>(),
            (&mut count as *mut u32).cast::<c_void>(),
        ];
        let grid = count.div_ceil(TPB);
        cu(
            cuLaunchKernel(k_fire, grid, 1, 1, TPB, 1, 1, 0, std::ptr::null_mut(), params.as_mut_ptr(), std::ptr::null_mut()),
            "launch k_fire",
        )?;
    }

    let res = dtoh::<u64>(d.result, 2, "read result")?;
    if res[0] == 0 {
        return Err("mithril-gpu: run finished without delivering a result to ROOT".to_string());
    }
    Ok(res[1])
}
