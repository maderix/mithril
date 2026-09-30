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
#[repr(C)]
#[derive(Clone, Copy)]
struct CUmemLocation {
    kind: i32,
    id: i32,
}
const CU_MEM_LOCATION_TYPE_DEVICE: i32 = 1;
const CU_MEM_ATTACH_GLOBAL: u32 = 1;
/// An array block's word 1: length in bits 0..47, size class 48..55, flags 62..63 (engine.cu).
const ARR_LEN_MASK: u64 = (1 << 48) - 1;
const CU_MEM_ADVISE_SET_PREFERRED_LOCATION: i32 = 3;

extern "C" {
    fn cuInit(flags: u32) -> CUresult;
    fn cuEventCreate(event: *mut *mut c_void, flags: u32) -> CUresult;
    fn cuEventRecord(event: *mut c_void, stream: *mut c_void) -> CUresult;
    fn cuEventElapsedTime(ms: *mut f32, start: *mut c_void, end: *mut c_void) -> CUresult;
    fn cuEventDestroy_v2(event: *mut c_void) -> CUresult;
    fn cuDeviceGet(device: *mut i32, ordinal: i32) -> CUresult;
    fn cuCtxSetLimit(limit: i32, value: usize) -> CUresult;
    fn cuStreamQuery(stream: *mut c_void) -> CUresult;
    fn cuModuleLoadData(module: *mut *mut c_void, image: *const c_void) -> CUresult;
    fn cuModuleGetFunction(f: *mut *mut c_void, module: *mut c_void, name: *const std::ffi::c_char) -> CUresult;
    fn cuModuleGetGlobal_v2(
        dptr: *mut CUdeviceptr,
        bytes: *mut usize,
        module: *mut c_void,
        name: *const std::ffi::c_char,
    ) -> CUresult;
    fn cuMemAlloc_v2(dptr: *mut CUdeviceptr, bytesize: usize) -> CUresult;
    fn cuMemAllocManaged(dptr: *mut CUdeviceptr, bytesize: usize, flags: u32) -> CUresult;
    fn cuMemAdvise_v2(dptr: CUdeviceptr, count: usize, advice: i32, location: CUmemLocation) -> CUresult;
    fn cuDevicePrimaryCtxRetain(ctx: *mut *mut c_void, dev: i32) -> CUresult;
    fn cuDevicePrimaryCtxRelease_v2(dev: i32) -> CUresult;
    fn cuDevicePrimaryCtxReset_v2(dev: i32) -> CUresult;
    fn cuCtxSynchronize() -> CUresult;
    fn cuMemFree_v2(dptr: CUdeviceptr) -> CUresult;
    fn cuModuleUnload(module: *mut c_void) -> CUresult;
    fn cuCtxSetCurrent(ctx: *mut c_void) -> CUresult;
    fn cuMemGetInfo_v2(free: *mut usize, total: *mut usize) -> CUresult;
    fn cuDeviceGetAttribute(pi: *mut i32, attrib: i32, dev: i32) -> CUresult;
    fn cuOccupancyMaxActiveBlocksPerMultiprocessor(n: *mut i32, f: *mut c_void, block: i32, shared: usize) -> CUresult;
    #[allow(clippy::too_many_arguments)]
    fn cuLaunchCooperativeKernel(
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
    ) -> CUresult;
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
const CU_DEVICE_ATTRIBUTE_CONCURRENT_MANAGED_ACCESS: i32 = 89;

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
    rc: CUdeviceptr,
    recs: CUdeviceptr,
    nbump: CUdeviceptr,
    rbump: CUdeviceptr,
    nfreen: CUdeviceptr,
    nchunk: CUdeviceptr,
    ebuf: CUdeviceptr,
    blen: CUdeviceptr,
    bdone: CUdeviceptr,
    result: CUdeviceptr,
    abortf: CUdeviceptr,
    heap: CUdeviceptr,
    hbump: CUdeviceptr,
    hcap: u64,
    nw: CUdeviceptr,
    nwn: CUdeviceptr,
    labels: CUdeviceptr,
    rfreen: CUdeviceptr,
    lstk: CUdeviceptr,
    lsn: CUdeviceptr,
    ncap: u32,
    rcap: u32,
    bcap: u32,
    chunksz: u32,
    nrules: u32,
    fuel: i32,
    net_fuel: i32,
}
const NWCAP: usize = 64;
const LSCAP: usize = 64;

/// What a run delivered to ROOT: the port, and its printed form (the same
/// text the CPU program prints).
#[derive(Clone, Debug, PartialEq)]
pub struct GpuResult {
    pub port: u64,
    pub text: String,
    /// rounds of the device driver (grow sweeps + work phases): the
    /// schedule's length, bounded by the program's fork levels
    pub rounds: u64,
    /// Cell arena bytes transferred to format the result (arrays are separate).
    pub cell_readback_bytes: usize,
}

const REC_SIZE: usize = 40; // sizeof(Rec) in engine.cu
const MAXLANES: usize = 1 << 16;
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
/// hit), load it through the driver API and run it (`k_run`). Returns the
/// result port raw delivered to ROOT.
pub fn compile_and_run(cu_src: &str, boot: Redex, cache_dir: &Path) -> Result<GpuResult, String> {
    run_cubin(&compile_to_cubin(cu_src, cache_dir)?, boot)
}

/// The per-thread stack a compiled program needed, kept beside it (a run
/// that hits the stack guard doubles it; the next run starts there). The
/// name extends the artefact's, so it is never the artefact itself.
fn stack_hint(cubin_path: &Path) -> std::path::PathBuf {
    let mut p = cubin_path.as_os_str().to_owned();
    p.push(".stack");
    p.into()
}

/// The cached .cubin of a generated program (compiled on a cache miss).
pub fn compile_to_cubin(cu_src: &str, cache_dir: &Path) -> Result<std::path::PathBuf, String> {
    // MITHRIL_GPU_CU=<file>: run a hand-edited program.cu instead (the SOP's
    // proof step before a change becomes a lowering)
    let edited = std::env::var("MITHRIL_GPU_CU").ok().map(|p| fs::read_to_string(&p).map_err(|e| format!("mithril-gpu: read {p}: {e}"))).transpose()?;
    let cu_src = edited.as_deref().unwrap_or(cu_src);
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
    Ok(cubin_path)
}

/// Run a compiled program (a prebuilt artefact: no front end).
pub fn run_cubin(cubin_path: &Path, boot: Redex) -> Result<GpuResult, String> {
    let cubin = fs::read(cubin_path).map_err(|e| format!("mithril-gpu: read cubin: {e}"))?;
    let t0 = std::time::Instant::now();
    let hint = stack_hint(cubin_path);
    let start = fs::read_to_string(&hint).ok().and_then(|t| t.trim().parse().ok());
    let (r, used) = GpuRunner::run_with_stack(&cubin, boot, start);
    // a size the user fixed (MITHRIL_GPU_STACK) is not the program's need
    if r.is_ok() && start != Some(used) && std::env::var_os("MITHRIL_GPU_STACK").is_none() {
        let _ = fs::write(&hint, used.to_string());
    }
    if std::env::var_os("MITHRIL_GPU_STATS").is_some() {
        eprintln!("mithril-gpu: context create + run + destroy {:.0} ms", t0.elapsed().as_secs_f64() * 1e3);
    }
    r
}

/// Set by a process that exits right after its one run (`mithril exec`,
/// `mithril run --gpu`): a successful run's buffers, module and context
/// are left to the driver's exit path (~150 ms saved).
pub static EXITING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A run of this process was abandoned at twice its deadline: its kernel
/// still runs, so no later run can start.
static STUCK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One run at a time: the kernel is cooperative over the whole device, and
/// every run shares the device's primary context.
static ONE_RUN: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A retain of the device's primary context, released when dropped: while
/// one is held, a run's release does not destroy the context, so memory a
/// run fails to free stays visible (`free_vram`).
pub struct ContextHold(i32);

pub fn hold_context() -> Result<ContextHold, String> {
    unsafe {
        cu(cuInit(0), "cuInit")?;
        let mut dev = 0i32;
        cu(cuDeviceGet(&mut dev, 0), "cuDeviceGet")?;
        let mut ctx: *mut c_void = std::ptr::null_mut();
        cu(cuDevicePrimaryCtxRetain(&mut ctx, dev), "cuDevicePrimaryCtxRetain")?;
        Ok(ContextHold(dev))
    }
}

impl Drop for ContextHold {
    fn drop(&mut self) {
        unsafe {
            let _ = cuDevicePrimaryCtxRelease_v2(self.0);
        }
    }
}

/// Free device memory in bytes, as the runner sees it between runs.
pub fn free_vram() -> Result<usize, String> {
    let _one = ONE_RUN.lock().unwrap_or_else(|e| e.into_inner());
    if STUCK.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("mithril-gpu: an earlier run of this process was abandoned and still occupies the device".into());
    }
    unsafe {
        cu(cuInit(0), "cuInit")?;
        let mut dev = 0i32;
        cu(cuDeviceGet(&mut dev, 0), "cuDeviceGet")?;
        let mut ctx: *mut c_void = std::ptr::null_mut();
        cu(cuDevicePrimaryCtxRetain(&mut ctx, dev), "cuDevicePrimaryCtxRetain")?;
        let r = cu(cuCtxSetCurrent(ctx), "cuCtxSetCurrent").and_then(|_| {
            let (mut free, mut total) = (0usize, 0usize);
            cu(cuMemGetInfo_v2(&mut free, &mut total), "cuMemGetInfo").map(|_| free)
        });
        let _ = cuDevicePrimaryCtxRelease_v2(dev);
        r
    }
}

/// Driver-API runner: loads a compiled .cubin and drives the wave loop.
pub struct GpuRunner;

impl GpuRunner {
    pub fn run(cubin: &[u8], boot: Redex) -> Result<GpuResult, String> {
        Self::run_with_stack(cubin, boot, None).0
    }

    /// Run with a starting per-thread stack; returns the stack that worked.
    pub fn run_with_stack(cubin: &[u8], boot: Redex, start: Option<usize>) -> (Result<GpuResult, String>, usize) {
        let mut used = 0;
        let r = unsafe { Self::run_inner(cubin, boot, start, &mut used) };
        (r, used)
    }

    unsafe fn run_inner(cubin: &[u8], boot: Redex, start: Option<usize>, used: &mut usize) -> Result<GpuResult, String> {
        let _one = ONE_RUN.lock().unwrap_or_else(|e| e.into_inner());
        if STUCK.load(std::sync::atomic::Ordering::Relaxed) {
            // anything queued now would wait behind the abandoned kernel
            return Err("mithril-gpu: an earlier run of this process was abandoned and still occupies the device".into());
        }
        cu(cuInit(0), "cuInit")?;
        let mut dev = 0i32;
        cu(cuDeviceGet(&mut dev, 0), "cuDeviceGet")?;
        // The per-thread stack is backed with local memory for every
        // resident thread, so its size is a fixed cost of every run
        // (nbody: 0.18 s wall at 32 KiB, 0.10 s at 8 KiB). It starts at
        // 8 KiB and doubles when the stack guard aborts the run. An aborted
        // run has no observable effect (its result is discarded and the
        // retry starts from fresh buffers), so the retry is sound. Doubling
        // stops where the device cannot back a deeper stack (the driver
        // refuses the limit, or the arenas no longer fit): the run then
        // reports the depth error of the last size that could run.
        let fixed = std::env::var_os("MITHRIL_GPU_STACK").is_some();
        let mut stack = if fixed { env_cap("MITHRIL_GPU_STACK", 8 * 1024) as usize } else { start.unwrap_or(8 * 1024).max(8 * 1024) };
        let mut deep: Option<String> = None;
        loop {
            *used = stack;
            let mut ctx: *mut c_void = std::ptr::null_mut();
            let tc = std::time::Instant::now();
            cu(cuDevicePrimaryCtxRetain(&mut ctx, dev), "cuDevicePrimaryCtxRetain")?;
            let r = match cu(cuCtxSetCurrent(ctx), "cuCtxSetCurrent") {
                Ok(()) => {
                    if std::env::var_os("MITHRIL_GPU_STATS").is_some() { eprintln!("mithril-gpu: context {:.0} ms", tc.elapsed().as_secs_f64() * 1e3); }
                    run_in_ctx(cubin, boot, dev, stack)
                }
                Err(e) => Err(e),
            };
            let exiting = r.is_ok() && EXITING.load(std::sync::atomic::Ordering::Relaxed);
            if !exiting {
                // an abandoned kernel still runs: nothing here would return
                if matches!(&r, Err(e) if e.starts_with(TIMEOUT) && e.ends_with(ABANDONED)) {
                    STUCK.store(true, std::sync::atomic::Ordering::Relaxed);
                    return r;
                }
                // a device fault (700) leaves a sticky error in the context,
                // which every later call reports: only then is it reset. A
                // clean abort (a stop at the deadline too) leaves it usable.
                if r.is_err() && cuCtxSynchronize() != 0 {
                    let _ = cuDevicePrimaryCtxReset_v2(dev);
                }
                let td = std::time::Instant::now();
                let _ = cuDevicePrimaryCtxRelease_v2(dev);
                if std::env::var_os("MITHRIL_GPU_STATS").is_some() { eprintln!("mithril-gpu: context release {:.0} ms", td.elapsed().as_secs_f64() * 1e3); }
            }
            match r {
                Err(e) if e.starts_with(DEEP) && !fixed => {
                    deep = Some(e);
                    stack *= 2;
                    if std::env::var_os("MITHRIL_GPU_STATS").is_some() { eprintln!("mithril-gpu: stack guard hit; re-running with {stack} bytes of stack per thread"); }
                }
                // the deeper stack could not be backed: the depth error stands
                Err(e) if deep.is_some() && cannot_back(&e) => {
                    *used = stack / 2;
                    return Err(deep.unwrap_or_default());
                }
                r => return r,
            }
        }
    }
}

/// What one run allocated on the device: freed (and the module unloaded)
/// when the run ends, so the primary context holds nothing between runs.
/// A successful run of an exiting process leaves it to the driver.
struct Mem {
    bufs: Vec<CUdeviceptr>,
    events: Vec<*mut c_void>,
    module: *mut c_void,
    /// a successful run of an exiting process: the driver reclaims it
    keep: bool,
    /// the kernel still runs (a timeout): the process's exit reclaims it
    abandon: bool,
}

impl Drop for Mem {
    fn drop(&mut self) {
        if self.keep || self.abandon {
            return;
        }
        unsafe {
            for &e in &self.events { let _ = cuEventDestroy_v2(e); }
            for &b in &self.bufs {
                let _ = cuMemFree_v2(b);
            }
            if !self.module.is_null() {
                let _ = cuModuleUnload(self.module);
            }
        }
    }
}

/// How a buffer's pages are committed.
#[derive(Clone, Copy, PartialEq)]
enum Commit {
    /// now: a per-lane table every lane writes as the kernel starts
    /// (demand paging would only move the faults into the kernel)
    Now,
    /// on first touch: an arena that fills from its start as the run
    /// allocates, so a run pays for what it uses, not for the arena's size
    OnTouch,
    /// managed with no preferred location: the host writes it while the
    /// kernel runs (the stop request at the deadline)
    Shared,
}

/// Allocate a device buffer. `OnTouch` is managed memory preferring the
/// device: address space is reserved at once and pages are committed on
/// first touch (committing every arena eagerly cost ~0.25 s of setup and
/// teardown on every run). `MITHRIL_GPU_EAGER=1` commits everything now (a
/// diagnostic: it separates demand-paging cost from the rest).
unsafe fn alloc(mem: &mut Mem, dev: i32, n: usize, commit: Commit, what: &str) -> Result<CUdeviceptr, String> {
    let mut p: CUdeviceptr = 0;
    if commit == Commit::Now || std::env::var_os("MITHRIL_GPU_EAGER").is_some() {
        cu(cuMemAlloc_v2(&mut p, n.max(1)), what)?;
        mem.bufs.push(p);
        return Ok(p);
    }
    cu(cuMemAllocManaged(&mut p, n.max(1), CU_MEM_ATTACH_GLOBAL), what)?;
    mem.bufs.push(p);
    if commit == Commit::Shared {
        return Ok(p);
    }
    let at = CUmemLocation { kind: CU_MEM_LOCATION_TYPE_DEVICE, id: dev };
    cu(cuMemAdvise_v2(p, n.max(1), CU_MEM_ADVISE_SET_PREFERRED_LOCATION, at), what)?;
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

/// The abort message of a run the stack guard stopped (see `GpuRunner::run`).
const DEEP: &str = "mithril-gpu: recursion too deep";
/// The error of a run stopped at its deadline.
const TIMEOUT: &str = "mithril-gpu: the device run exceeded";
/// The end of the error of a run that did not stop by twice its deadline.
const ABANDONED: &str = "and did not stop; abandoned until the process exits";
/// The abort code the host writes at the deadline (engine.cu `AB_TIMEOUT`).
const AB_TIMEOUT: u32 = 12;

/// The device could not back a run: its stack limit or its buffers.
fn cannot_back(e: &str) -> bool {
    e.contains("cuCtxSetLimit") || e.contains("call alloc ") || e.contains("arena exhausted")
}

unsafe fn run_in_ctx(cubin: &[u8], boot: Redex, dev: i32, stack: usize) -> Result<GpuResult, String> {
    let t0 = std::time::Instant::now();
    cu(cuCtxSetLimit(CU_LIMIT_STACK_SIZE, stack), "cuCtxSetLimit(stack)")?;

    let mut mem = Mem { bufs: Vec::new(), events: Vec::new(), module: std::ptr::null_mut(), keep: false, abandon: false };
    cu(cuModuleLoadData(&mut mem.module, cubin.as_ptr() as *const c_void), "cuModuleLoadData")?;
    let module = mem.module;
    let t_load = t0.elapsed();

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
        // a GROW sweep pushes a whole level at once: the rings hold it
        ((1u64 << 27) / nrules as u64).clamp(1 << 14, 1 << 21) as u32
    };
    let bcap = bcap.next_power_of_two(); // rings: a power of two
    let hcap_req = std::env::var("MITHRIL_GPU_HEAP").ok().map(|_| env_cap("MITHRIL_GPU_HEAP", 1 << 26));
    let fuel = env_cap("MITHRIL_GPU_FUEL", 64).clamp(1, i32::MAX as u64) as i32;
    let net_fuel = env_cap("MITHRIL_GPU_NET_FUEL", 4096).clamp(1, i32::MAX as u64) as i32;

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
        + (12 * MAXLANES) as u64
        + 8 * nrules as u64
        + (16 * NWCAP * MAXLANES + 4 * MAXLANES + 4 + 4 * MAXLANES + 32 * LSCAP * MAXLANES + 4 * MAXLANES) as u64
        + (1 << 20);
    let slack: u64 = 1 << 30;
    let budget0 = (vfree as u64).saturating_sub(fixed + stack_reserve + slack);
    // the array heap takes a third of what is left (a program is arrays or
    // cells; neither arena is sized by the program: a parallel array
    // program holds one working set per lane), the cells the rest
    // block indices are u32 on the free lists; below 2^20 words the heap is
    // smaller than one lane's working set
    let hcap = hcap_req.unwrap_or(budget0 / 24).clamp(1 << 20, 1 << 32);
    if hcap_req.is_some_and(|h| h != hcap) {
        eprintln!("mithril-gpu: warning: MITHRIL_GPU_HEAP={} is outside [2^20, 2^32] words; using {hcap}", hcap_req.unwrap_or(0));
    }
    let budget = budget0.saturating_sub(8 * hcap);
    let max_ncap = (budget / 20).max(1 << 10).min(u32::MAX as u64 - 1);
    let ncap = if ncap_req > max_ncap {
        // the default asks for more than most devices have: only a value the
        // user set is worth a warning
        if std::env::var_os("MITHRIL_GPU_NODES").is_some() {
            eprintln!(
                "mithril-gpu: warning: MITHRIL_GPU_NODES={ncap_req} does not fit in free VRAM \
                 ({} MiB); capping the cell arena to {max_ncap} cells",
                vfree >> 20
            );
        }
        max_ncap as u32
    } else {
        ncap_req as u32
    };
    let chunksz = ((ncap as u64) / 65536).clamp(2, 4096) as u32;

    // device buffers
    let d = Dev {
        nodes: alloc(&mut mem, dev, 16 * ncap as usize, Commit::OnTouch, "alloc nodes")?,
        rc: alloc(&mut mem, dev, 4 * ncap as usize, Commit::OnTouch, "alloc rc")?,
        recs: alloc(&mut mem, dev, REC_SIZE * rcap as usize, Commit::OnTouch, "alloc recs")?,
        nbump: alloc(&mut mem, dev, 4, Commit::Now, "alloc nbump")?,
        rbump: alloc(&mut mem, dev, 4, Commit::Now, "alloc rbump")?,
        nfreen: alloc(&mut mem, dev, 4 * MAXLANES, Commit::Now, "alloc nfreen")?,
        nchunk: alloc(&mut mem, dev, 8 * MAXLANES, Commit::Now, "alloc nchunk")?,
        ebuf: alloc(&mut mem, dev, 24 * bcap as usize * nrules, Commit::OnTouch, "alloc ebuf")?,
        blen: alloc(&mut mem, dev, 4 * nrules, Commit::Now, "alloc blen")?,
        bdone: alloc(&mut mem, dev, 4 * nrules, Commit::Now, "alloc bdone")?,
        result: alloc(&mut mem, dev, 16, Commit::Now, "alloc result")?,
        abortf: alloc(&mut mem, dev, 4, Commit::Shared, "alloc abortf")?,
        heap: alloc(&mut mem, dev, 8 * hcap as usize, Commit::OnTouch, "alloc heap")?,
        hbump: alloc(&mut mem, dev, 8, Commit::Now, "alloc hbump")?,
        hcap,
        nw: alloc(&mut mem, dev, 16 * NWCAP * MAXLANES, Commit::Now, "alloc nw")?,
        nwn: alloc(&mut mem, dev, 4 * MAXLANES, Commit::Now, "alloc nwn")?,
        labels: alloc(&mut mem, dev, 4, Commit::Now, "alloc labels")?,
        rfreen: alloc(&mut mem, dev, 4 * MAXLANES, Commit::Now, "alloc rfreen")?,
        lstk: alloc(&mut mem, dev, 32 * LSCAP * MAXLANES, Commit::Now, "alloc lstk")?,
        lsn: alloc(&mut mem, dev, 4 * MAXLANES, Commit::Now, "alloc lsn")?,
        ncap,
        rcap,
        bcap,
        chunksz,
        nrules: nrules as u32,
        fuel,
        net_fuel,
    };
    if std::env::var_os("MITHRIL_GPU_DEBUG").is_some() {
        eprintln!("mithril-gpu: nodes {:#x}+{:#x} rc {:#x}+{:#x} recs {:#x}+{:#x} ebuf {:#x}+{:#x} heap {:#x}+{:#x} nw {:#x}+{:#x}",
            d.nodes, 16 * ncap as u64, d.rc, 4 * ncap as u64, d.recs, REC_SIZE as u64 * rcap as u64, d.ebuf, 24 * bcap as u64 * nrules as u64, d.heap, 8 * hcap, d.nw, (16 * NWCAP * MAXLANES) as u64);
    }
    cu(cuMemsetD8_v2(d.nodes, 0, 16), "memset cell0")?;
    cu(cuMemsetD8_v2(d.nfreen, 0, 4 * MAXLANES), "memset nfreen")?;
    cu(cuMemsetD8_v2(d.nchunk, 0, 8 * MAXLANES), "memset nchunk")?;
    cu(cuMemsetD8_v2(d.blen, 0, 4 * nrules), "memset blen")?;
    cu(cuMemsetD8_v2(d.bdone, 0, 4 * nrules), "memset bdone")?;
    cu(cuMemsetD8_v2(d.result, 0, 16), "memset result")?;
    cu(cuMemsetD8_v2(d.abortf, 0, 4), "memset abortf")?;
    let one: u32 = 1; // cell 0 and record 0 are reserved
    cu(cuMemcpyHtoD_v2(d.nbump, (&one as *const u32).cast(), 4), "init nbump")?;
    cu(cuMemcpyHtoD_v2(d.rbump, (&one as *const u32).cast(), 4), "init rbump")?;
    // heap words 0..2: an empty array that is never freed, the block an
    // allocation that aborts on a full heap returns (its length is 0, so
    // no caller writes into it)
    let empty: [u64; 2] = [1 << 40, 0];
    cu(cuMemcpyHtoD_v2(d.heap, empty.as_ptr().cast(), 16), "init heap")?;
    let two64: u64 = 2;
    cu(cuMemcpyHtoD_v2(d.hbump, (&two64 as *const u64).cast(), 8), "init hbump")?;
    // no clear of the refcounts: every allocation of a refcounted value
    // writes its count (alloc2, mk_con), and clearing would commit the
    // whole managed arena
    // MITHRIL_GPU_POISON=<buffers>: fill never-initialized buffers with a
    // pattern so a read of unwritten memory is deterministic (a probe)
    if let Ok(pz) = std::env::var("MITHRIL_GPU_POISON") {
        if pz.contains("nodes") { cu(cuMemsetD8_v2(d.nodes, 0xCD, 16 * ncap as usize), "poison nodes")?; cu(cuMemsetD8_v2(d.nodes, 0, 16), "memset cell0")?; }
        if pz.contains("recs") { cu(cuMemsetD8_v2(d.recs, 0xCD, REC_SIZE * rcap as usize), "poison recs")?; }
        if pz.contains("ebuf") { cu(cuMemsetD8_v2(d.ebuf, 0xCD, 24 * bcap as usize * nrules), "poison ebuf")?; }
        if pz.contains("heap") { cu(cuMemsetD8_v2(d.heap, 0xCD, 8 * hcap as usize), "poison heap")?; }
    }
    cu(cuMemsetD8_v2(d.nwn, 0, 4 * MAXLANES), "memset nwn")?;
    cu(cuMemsetD8_v2(d.rfreen, 0, 4 * MAXLANES), "memset rfreen")?;
    cu(cuMemsetD8_v2(d.lsn, 0, 4 * MAXLANES), "memset lsn")?;
    cu(cuMemcpyHtoD_v2(d.labels, (&one as *const u32).cast(), 4), "init labels")?;

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

    {
        // the stack guard's limit, set before any launch (k_boot runs dives
        // too): the thread's stack less a margin for the
        // runtime's own frames below the guard
        let lim: u32 = stack.saturating_sub(4096).max(1024) as u32;
        let (mut p, mut sz) = (0 as CUdeviceptr, 0usize);
        cu(cuModuleGetGlobal_v2(&mut p, &mut sz, module, c"g_stack_limit".as_ptr()), "cuModuleGetGlobal(g_stack_limit)")?;
        cu(cuMemcpyHtoD_v2(p, (&lim as *const u32).cast(), 4), "set g_stack_limit")?;
    }
    let mut k_boot: *mut c_void = std::ptr::null_mut();
    cu(cuModuleGetFunction(&mut k_boot, module, c"k_boot".as_ptr()), "get k_boot")?;

    let stats = std::env::var_os("MITHRIL_GPU_STATS").is_some();
    if stats {
        for _ in 0..4 {
            let mut event = std::ptr::null_mut();
            cu(cuEventCreate(&mut event, 0), "create timing event")?;
            mem.events.push(event);
        }
        cu(cuEventRecord(mem.events[0], std::ptr::null_mut()), "record boot start")?;
    }
    // which rules can fork (published by the program)
    let t_setup = t0.elapsed();
    // boot fires rule 0 with the redex, parent = ROOT (aux), in the
    // parallel world with the dive budget
    {
        let (mut a, mut b, mut c) = (boot.a, boot.b, boot.aux);
        let mut bf: i32 = fuel;
        let mut params = [
            (&mut a as *mut u64).cast::<c_void>(),
            (&mut b as *mut u64).cast::<c_void>(),
            (&mut c as *mut u64).cast::<c_void>(),
            (&mut bf as *mut i32).cast::<c_void>(),
        ];
        cu(
            cuLaunchKernel(k_boot, 1, 1, 1, 1, 1, 1, 0, std::ptr::null_mut(), params.as_mut_ptr(), std::ptr::null_mut()),
            "launch k_boot",
        )?;
    }

    let t_boot = t0.elapsed();
    if stats { cu(cuEventRecord(mem.events[1], std::ptr::null_mut()), "record boot end")?; }
    // The driver on the device: k_run, one cooperative launch.
    let abort_message = |ab: u32| -> String {
        match ab {
            1 => "mithril-gpu: unreachable match arm reached".to_string(),
            3 => "mithril-gpu: array index out of bounds".to_string(),
            4 => "mithril-gpu: the program used an unsupported device feature".to_string(),
            5 => "mithril-gpu: a cell walk did not terminate (corrupted arena)".to_string(),
            6 => format!("mithril-gpu: arena exhausted: records ({rcap}; raise MITHRIL_GPU_RECS)"),
            7 => format!("mithril-gpu: arena exhausted: rule bucket ({bcap} entries; raise MITHRIL_GPU_BUCKET)"),
            8 => format!("mithril-gpu: arena exhausted: array heap ({hcap} words; raise MITHRIL_GPU_HEAP)"),
            9 => "mithril-gpu: a cell index outside the arena was read (corrupted port)".to_string(),
            10 => "mithril-gpu: round limit reached (MITHRIL_GPU_ROUNDS): the run does not converge".to_string(),
            11 => format!("{DEEP} for the device ({} bytes of stack per thread; MITHRIL_GPU_STACK)", stack),
            _ => format!("mithril-gpu: arena exhausted: cells ({ncap}; raise MITHRIL_GPU_NODES)"),
        }
    };
    let mut k_run: *mut c_void = std::ptr::null_mut();
    cu(cuModuleGetFunction(&mut k_run, module, c"k_run".as_ptr()), "get k_run")?;
    let mut per_sm: i32 = 0;
    cu(cuOccupancyMaxActiveBlocksPerMultiprocessor(&mut per_sm, k_run, TPB as i32, 0), "occupancy k_run")?;
    let mut sms: i32 = 0;
    cu(cuDeviceGetAttribute(&mut sms, CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT, dev), "sm count")?;
    // MITHRIL_GPU_LANES caps the lanes (the one-lane run is the determinism probe)
    let blocks = ((per_sm.max(1) * sms.max(1)) as u32).min(MAXLANES as u32 / TPB).min(env_cap("MITHRIL_GPU_LANES", MAXLANES as u64).clamp(1, MAXLANES as u64).div_ceil(TPB as u64) as u32).max(1);
    let lanes = blocks * TPB;
    let t_prepare = t0.elapsed();
    if stats { cu(cuEventRecord(mem.events[2], std::ptr::null_mut()), "record run start")?; }
    let t_run = std::time::Instant::now();
    let mut width: u32 = env_cap("MITHRIL_GPU_GROW_WIDTH", lanes as u64).clamp(1, lanes as u64) as u32;
    let mut steps: u32 = env_cap("MITHRIL_GPU_WORK_STEPS", 1 << 30).clamp(1, u32::MAX as u64) as u32;
    // a grow task runs its body with the dive budget; fork-site callees
    // and tail calls become tasks at once
    let mut gfuel: i32 = env_cap("MITHRIL_GPU_GROW_FUEL", fuel as u64).clamp(1, i32::MAX as u64) as i32;
    // a run that does not converge stops with an error at this many rounds
    let mut max_rounds: u64 = env_cap("MITHRIL_GPU_ROUNDS", 1 << 24);
    let mut params = [(&mut width as *mut u32).cast::<c_void>(), (&mut steps as *mut u32).cast::<c_void>(), (&mut gfuel as *mut i32).cast::<c_void>(), (&mut max_rounds as *mut u64).cast::<c_void>()];
    cu(
        cuLaunchCooperativeKernel(k_run, blocks, 1, 1, TPB, 1, 1, 0, std::ptr::null_mut(), params.as_mut_ptr()),
        "launch k_run (cooperative)",
    )?;
    if stats { cu(cuEventRecord(mem.events[3], std::ptr::null_mut()), "record run end")?; }
    // wait with a deadline: a run past MITHRIL_GPU_TIMEOUT seconds is an
    // error; lanes that do not stop (a native loop, a runaway forking
    // recursion) leave the kernel running until the process exits
    let deadline = std::time::Duration::from_secs(env_cap("MITHRIL_GPU_TIMEOUT", 300));
    let mut stop_sent = false;
    let mut concurrent = 0i32;
    cu(cuDeviceGetAttribute(&mut concurrent, CU_DEVICE_ATTRIBUTE_CONCURRENT_MANAGED_ACCESS, dev), "attr(concurrent managed access)")?;
    let concurrent = concurrent != 0;
    loop {
        let r = cuStreamQuery(std::ptr::null_mut());
        if r == 0 {
            break;
        }
        if r != 600 {
            return Err(format!("mithril-gpu: the device run failed with code {r}"));
        }
        if t_run.elapsed() > deadline && !stop_sent {
            // ask the lanes to stop: the abort flag, which every lane reads
            // at its next scheduler check (a native loop never checks)
            // (only where the host may write managed memory while a kernel
            // runs, and only when no abort is set: the first abort wins)
            if concurrent && std::ptr::read_volatile(d.abortf as *const u32) == 0 {
                std::ptr::write_volatile(d.abortf as *mut u32, AB_TIMEOUT);
            }
            stop_sent = true;
        }
        if t_run.elapsed() > 2 * deadline {
            // the lanes did not stop: the kernel still runs, and freeing,
            // releasing or resetting would each wait for it; the run is
            // abandoned (the process's exit ends the kernel)
            mem.abandon = true;
            return Err(format!("{TIMEOUT} {} s (MITHRIL_GPU_TIMEOUT) {ABANDONED}", deadline.as_secs()));
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let ab = dtoh::<u32>(d.abortf, 1, "read abortf")?[0];
    if ab == AB_TIMEOUT {
        return Err(format!("{TIMEOUT} {} s (MITHRIL_GPU_TIMEOUT); stopped", deadline.as_secs()));
    }
    let mut ptr: CUdeviceptr = 0;
    let mut sz = 0usize;
    cu(cuModuleGetGlobal_v2(&mut ptr, &mut sz, module, c"g_rounds".as_ptr()), "cuModuleGetGlobal(g_rounds)")?;
    let r = dtoh::<u64>(ptr, 8, "read g_rounds")?;
    if stats {
        let (mut boot_ms, mut run_ms) = (0f32, 0f32);
        cu(cuEventElapsedTime(&mut boot_ms, mem.events[0], mem.events[1]), "time boot")?;
        cu(cuEventElapsedTime(&mut run_ms, mem.events[2], mem.events[3]), "time run")?;
        eprintln!("mithril-gpu: device events: boot {boot_ms:.3} ms, run {run_ms:.3} ms");
        let rb = dtoh::<u32>(d.rbump, 1, "read rbump")?[0];
        let nb = dtoh::<u32>(d.nbump, 1, "read nbump")?[0];
        eprintln!("mithril-gpu: setup {:.0} ms, run {:.0} ms on {lanes} lanes: {} rounds ({} grow sweeps {:.0} M cycles, {} work phases {:.0} M cycles, widest frontier {}), {nb} cells and {rb} records issued", t_setup.as_secs_f64() * 1e3, t_run.elapsed().as_secs_f64() * 1e3, r[0], r[1], r[4] as f64 / 1e6, r[2], r[5] as f64 / 1e6, r[3]);
        if std::env::var_os("MITHRIL_GPU_TRACE").is_some() {
            cu(cuModuleGetGlobal_v2(&mut ptr, &mut sz, module, c"g_whist".as_ptr()), "cuModuleGetGlobal(g_whist)")?;
            let h = dtoh::<u32>(ptr, 40, "read g_whist")?;
            let hs: Vec<String> = (0..40).filter(|k| h[*k] > 0).map(|k| format!("2^{k}K:{}", h[k])).collect();
            eprintln!("mithril-gpu: last work phase, lanes by cycles: {}", hs.join(" "));
            let n = (r[0] as usize).min(1 << 20);
            cu(cuModuleGetGlobal_v2(&mut ptr, &mut sz, module, c"g_log".as_ptr()), "cuModuleGetGlobal(g_log)")?;
            let log = dtoh::<u32>(ptr, n * 3, "read g_log")?;
            cu(cuModuleGetGlobal_v2(&mut ptr, &mut sz, module, c"g_wlog".as_ptr()), "cuModuleGetGlobal(g_wlog)")?;
            let wl = dtoh::<u32>(ptr, n * 6, "read g_wlog")?;
            cu(cuModuleGetGlobal_v2(&mut ptr, &mut sz, module, c"g_rlog".as_ptr()), "cuModuleGetGlobal(g_rlog)")?;
            let rl = dtoh::<u32>(ptr, 64 * nrules, "read g_rlog")?;
            for i in 0..n {
                if i < 64 {
                    let per: Vec<String> = (0..nrules).filter(|k| rl[i * nrules + k] > 0).map(|k| format!("r{k}:{}", rl[i * nrules + k])).collect();
                    eprintln!("mithril-gpu:   [{}]", per.join(" "));
                }
                let ph = ["EXIT", "GROW", "WORK"][log[i * 3].min(2) as usize];
                let w = if log[i * 3] == 2 { format!(" steps max {} sum {} Kcycles {} slowest lane {} Kcycles in {} steps, {} lanes busy", wl[i * 6], wl[i * 6 + 1], wl[i * 6 + 2], wl[i * 6 + 3], wl[i * 6 + 4], wl[i * 6 + 5]) } else { format!(" Kcycles {}", wl[i * 6 + 2]) };
                eprintln!("mithril-gpu: round {i}: pending {} pushed {} -> {ph}{w}", log[i * 3 + 1], log[i * 3 + 2]);
            }
        }
    }
    if ab != 0 {
        return Err(abort_message(ab));
    }
    let t_before_read = t0.elapsed();
    let res = dtoh::<u64>(d.result, 2, "read result")?;
    if std::env::var_os("MITHRIL_GPU_STATS").is_some() {
        eprintln!("mithril-gpu: phases: module load {:.0} ms, arenas {:.0} ms, to readback {:.0} ms", t_load.as_secs_f64() * 1e3, (t_setup - t_load).as_secs_f64() * 1e3, (t_before_read - t_setup).as_secs_f64() * 1e3);
    }
    if res[0] == 0 {
        return Err("mithril-gpu: run finished without delivering a result to ROOT".to_string());
    }
    let mut ub_ptr: CUdeviceptr = 0;
    let mut ub_sz = 0usize;
    cu(cuModuleGetGlobal_v2(&mut ub_ptr, &mut ub_sz, module, c"UNBOX_CID".as_ptr()), "cuModuleGetGlobal(UNBOX_CID)")?;
    let unbox = dtoh::<u32>(ub_ptr, ub_sz / 4, "read UNBOX_CID")?;
    // Snapshot the cell arena only if the result walk reaches a cell.
    // Immediate values and arrays of immediate values need none of it.
    let mut cells = Vec::new();
    let text = show(&d, &mut cells, &unbox, res[1])?;
    if stats {
        eprintln!("mithril-gpu: detail: boot launch {:.3} ms, prepare run {:.3} ms, readback + format {:.3} ms ({} cell bytes), stack {stack}", (t_boot - t_setup).as_secs_f64() * 1e3, (t_prepare - t_boot).as_secs_f64() * 1e3, (t0.elapsed() - t_before_read).as_secs_f64() * 1e3, cells.len() * 8);
    }
    mem.keep = EXITING.load(std::sync::atomic::Ordering::Relaxed);
    Ok(GpuResult { port: res[1], text, rounds: r[0], cell_readback_bytes: cells.len() * 8 })
}

// ---- readback of a result port (mirrors mithril_rt::prelude::show) ----

const T_NUM: u64 = 2;
const T_FLO: u64 = 3;
const T_CON: u64 = 4;
const T_LAM: u64 = 6;
const T_ARR: u64 = 14;
const TU: u64 = 16;
const M56: u64 = (1u64 << 56) - 1;

unsafe fn cell(d: &Dev, cells: &mut Vec<u64>, i: u32) -> Result<[u64; 2], String> {
    if cells.is_empty() {
        // Keep bulk transfer for aggregate results: a transfer per cell
        // cost ~1.7 s for a 512 x 512 image. All later reads use this snapshot.
        let used = dtoh::<u32>(d.nbump, 1, "read nbump")?[0].min(d.ncap) as usize;
        *cells = dtoh::<u64>(d.nodes, 2 * used, "read cells")?;
    }
    let k = 2 * i as usize;
    cells.get(k..k + 2).map(|c| [c[0], c[1]]).ok_or_else(|| format!("mithril-gpu: result cell {i} is outside the cells in use"))
}

unsafe fn show(d: &Dev, cells: &mut Vec<u64>, unbox: &[u32], p: u64) -> Result<String, String> {
    let as_i = |p: u64| ((p << 8) as i64) >> 8;
    let t = p >> 56;
    Ok(match t {
        t if t >= TU => format!("C{}({})", unbox.get((t - TU) as usize).copied().unwrap_or(0), as_i(p)),
        T_LAM => "<closure>".to_string(),
        T_NUM => as_i(p).to_string(),
        T_FLO => format!("{:?}", f64::from_bits(cell(d, cells, (p & M56) as u32)?[0])),
        T_CON => {
            let k = ((p >> 4) & 0xFFF) as u16;
            let mut q = p;
            let mut fs: Vec<String> = Vec::new();
            if p & 0xF != 0 {
                loop {
                    let ar = (q & 0xF) as usize;
                    let c = cell(d, cells, ((q >> 16) & ((1u64 << 40) - 1)) as u32)?;
                    if ar > 2 {
                        fs.push(show(d, cells, unbox, c[0])?);
                        q = c[1];
                    } else {
                        for s in c.iter().take(ar) {
                            fs.push(show(d, cells, unbox, *s)?);
                        }
                        break;
                    }
                }
            }
            if k == 0xFFF { format!("({})", fs.join(", ")) } else { format!("C{}({})", k, fs.join(", ")) }
        }
        T_ARR => {
            let base = p & M56;
            let hdr = dtoh::<u64>(d.heap + 8 * base, 2, "read array header")?;
            let raw = hdr[1] & (1 << 62) != 0;
            let n = (hdr[1] & ARR_LEN_MASK) as usize;
            let elems = dtoh::<u64>(d.heap + 8 * (base + 2), n, "read array")?;
            let mut fs = Vec::new();
            for e in elems {
                let e = if raw { (e >> 8) | (T_NUM << 56) } else { e };
                fs.push(show(d, cells, unbox, e)?);
            }
            format!("[{}]", fs.join(", "))
        }
        _ => return Err(format!("mithril-gpu: unprintable result port {p:#x}")),
    })
}
