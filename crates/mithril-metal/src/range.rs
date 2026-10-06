//! Range requests on the GPU: a proven fold over `[lo, hi)` (a fill of an
//! array, or a wrapping sum) runs one index per thread through the
//! program's leaves (`msl/range.metal`, `mithril_codegen::cprint::msl_leaves`).
//!
//! The arrays a request passes are copied into one shared buffer and reach
//! the leaves as device addresses; after the dispatch a fill's array is
//! copied back. The call stack a pipeline declares is reserved for every
//! thread in flight, so its depth adapts: halved when the GPU runs out of
//! memory, doubled when a leaf nests deeper. A leaf that faults past that
//! (an index out of bounds, recursion past the most the stack holds) makes
//! [`RangeExec::run`] return `None`: the caller runs the request on the
//! CPU, whose result is the same by construction and which reports a fault.

use crate::metal::{Device, Dispatch, Library, Pipeline};
use std::sync::Mutex;

/// One argument of a request, in port order from index 2 (the fold's index
/// and bound are the range itself).
pub enum Arg<'a> {
    Int(i64),
    /// an int array's block (`[refcount, length | flags, elements...]`),
    /// written back when `write` (a fill's array)
    Array { block: &'a mut [u64], write: bool },
}

const KERNEL: &str = r#"
kernel void range_launch(device const ulong *req [[buffer(0)]], device ulong *parts [[buffer(1)]],
                         device metal::atomic_uint *fault [[buffer(2)]],
                         uint t [[thread_position_in_grid]], uint lt [[thread_position_in_threadgroup]],
                         uint g [[threadgroup_position_in_grid]], uint size [[threads_per_threadgroup]]) {
  threadgroup ulong part[1024];
  // req: fid, lo, n, the guarded depth the stack holds, then the fold's
  // ports from index 1 (args[k] = req[3 + k]; ports 0 and 1 are the range)
  ulong v = 0;
  if (t < req[2]) {
    i64 fuel[3] = {(i64)1 << 60, 0, (i64)req[3]};
    v = (ulong)prog_range_leaf((uint)req[0], (i64)req[1] + (i64)t, req + 3, fuel);
    if (fuel[1] != 0) atomic_store_explicit(fault, 1u, metal::memory_order_relaxed);
  }
  part[lt] = v;
  threadgroup_barrier(metal::mem_flags::mem_threadgroup);
  for (uint s = size / 2; s > 0; s >>= 1) {
    if (lt < s) part[lt] += part[lt + s];
    threadgroup_barrier(metal::mem_flags::mem_threadgroup);
  }
  if (lt == 0) parts[g] = part[0];
}
"#;

const T_ARR: u64 = 14;
/// The most call frames a Metal pipeline may declare.
const MAX_STACK: usize = 4096;
/// Frames below the leaves' own: the kernel, prog_range_leaf, and the
/// deepest chain of the scalar operations (the software float path:
/// sf_f32_op, sf_f64_op, sf_div, sf_round_pack, ...), with a margin.
const BELOW: usize = 32;
/// The guarded depth a program starts with.
const START_DEEP: usize = 16;

/// The GPU and the pipeline of one program's range leaves.
pub struct RangeExec {
    dev: Device,
    lib: Library,
    /// call frames per level of guarded recursion
    frames: usize,
    /// the guarded depth the current pipeline's stack holds, the pipeline,
    /// and its threadgroup size
    current: Mutex<(usize, Pipeline, usize)>,
    /// the least depth whose stack the GPU could not hold
    ceiling: std::sync::atomic::AtomicUsize,
}

enum Outcome {
    Done(u64),
    /// out of memory: the stack must shrink
    Memory,
    /// a leaf nested deeper than the stack holds
    Deep,
    /// out of bounds, or the GPU failed
    Fault,
}

fn trace(what: &str) {
    if std::env::var_os("MITHRIL_METAL_TRACE").is_some() {
        eprintln!("metal: {what}");
    }
}

impl RangeExec {
    /// Compile `leaves` (empty: the program has none the GPU can run).
    pub fn new(leaves: &str) -> Result<RangeExec, String> {
        let header = leaves.lines().next().unwrap_or("");
        let frames: usize = header.strip_prefix("// mithril: frames ").and_then(|n| n.trim().parse().ok()).ok_or("no range leaves for the GPU")?;
        let dev = Device::new().ok_or("no Metal device")?;
        let quiet = "#pragma clang diagnostic ignored \"-Wunused-variable\"".to_string();
        let src = [crate::ops_source(), quiet, include_str!("../msl/range.metal").to_string(), leaves.to_string(), KERNEL.to_string()].join("\n");
        // the compiler's report: its errors (generated code leaves unused locals)
        let t0 = std::time::Instant::now();
        let lib = dev.compile(&src).map_err(|e| e.lines().filter(|l| l.contains("error")).collect::<Vec<_>>().join("\n"))?;
        trace(&format!("compiled the leaves in {} ms", t0.elapsed().as_millis()));
        let deep = START_DEEP.min(Self::most(frames));
        let (pso, group) = Self::pipeline(&dev, &lib, frames, deep)?;
        let ceiling = std::sync::atomic::AtomicUsize::new(usize::MAX);
        Ok(RangeExec { dev, lib, frames, current: Mutex::new((deep, pso, group)), ceiling })
    }

    /// The deepest guarded nesting a full stack holds.
    fn most(frames: usize) -> usize {
        ((MAX_STACK - BELOW) / frames).saturating_sub(1)
    }

    fn pipeline(dev: &Device, lib: &Library, frames: usize, deep: usize) -> Result<(Pipeline, usize), String> {
        let pso = dev.pipeline(lib, "range_launch", (deep + 1) * frames + BELOW)?;
        // the reduction halves the group: a power of two
        let group = 1 << pso.max_group().min(1024).ilog2();
        Ok((pso, group))
    }

    /// Fold `fid` over `[lo, hi)`: the wrapping sum of the leaves (a fill's
    /// is meaningless), or `None` to run it on the CPU.
    pub fn run(&self, fid: u32, lo: i64, hi: i64, args: &mut [Arg]) -> Option<u64> {
        let n = (hi - lo).max(0) as usize;
        if n == 0 || n > u32::MAX as usize {
            return None;
        }
        use std::sync::atomic::Ordering::Relaxed;
        let mut cur = self.current.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            let deeper = (cur.0 * 2).min(Self::most(self.frames));
            let deep = match self.attempt(&cur, fid, lo, n, args) {
                Outcome::Done(s) => return Some(s),
                Outcome::Fault => return None,
                Outcome::Memory if cur.0 > 2 => {
                    self.ceiling.fetch_min(cur.0, Relaxed);
                    cur.0 / 2
                }
                Outcome::Deep if deeper > cur.0 && deeper < self.ceiling.load(Relaxed) => deeper,
                Outcome::Memory | Outcome::Deep => return None,
            };
            trace(&format!("call stack for {deep} nested guarded calls"));
            let (pso, group) = Self::pipeline(&self.dev, &self.lib, self.frames, deep).ok()?;
            *cur = (deep, pso, group);
        }
    }

    fn attempt(&self, cur: &(usize, Pipeline, usize), fid: u32, lo: i64, n: usize, args: &mut [Arg]) -> Outcome {
        let (deep, pso, group) = (cur.0, &cur.1, cur.2);
        let words: usize = args.iter().map(|a| if let Arg::Array { block, .. } = a { block.len() } else { 0 }).sum();
        let mut arrays = self.dev.buffer(words * 8);
        let base = arrays.gpu_address();
        let mut req = self.dev.buffer((5 + args.len()) * 8);
        let r = req.words_mut();
        r[..5].copy_from_slice(&[fid as u64, lo as u64, n as u64, deep as u64, 0]);
        let mut at = 0;
        let staged = arrays.words_mut();
        for (k, a) in args.iter().enumerate() {
            r[5 + k] = match a {
                Arg::Int(v) => *v as u64,
                Arg::Array { block, .. } => {
                    staged[at..at + block.len()].copy_from_slice(block);
                    // the refcount word becomes the fault mark
                    staged[at] = 0;
                    let handle = (T_ARR << 56) | (base + at as u64 * 8);
                    at += block.len();
                    handle
                }
            };
        }
        let groups = n.div_ceil(group);
        let parts = self.dev.buffer(groups * 8);
        let fault = self.dev.buffer(8);
        let d = Dispatch { pipeline: pso, buffers: vec![&req, &parts, &fault], reached: vec![&arrays], threads: groups * group, group };
        let t0 = std::time::Instant::now();
        let ran = self.dev.run(&[d]);
        trace(&format!("{n} indices at depth {deep}: {} ms", t0.elapsed().as_millis()));
        if let Err(e) = ran {
            trace(&e);
            return if e.contains("Memory") { Outcome::Memory } else { Outcome::Fault };
        }
        if fault.words()[0] != 0 {
            trace(&format!("a leaf nested deeper than {deep} guarded calls"));
            return Outcome::Deep;
        }
        let staged = arrays.words();
        let mut at = 0;
        for a in args.iter() {
            if let Arg::Array { block, .. } = a {
                if staged[at] != 0 {
                    trace("a leaf indexed an array out of bounds");
                    return Outcome::Fault;
                }
                at += block.len();
            }
        }
        let mut at = 0;
        for a in args.iter_mut() {
            if let Arg::Array { block, write } = a {
                let len = block.len();
                if *write {
                    block[2..].copy_from_slice(&staged[at + 2..at + len]);
                }
                at += len;
            }
        }
        Outcome::Done(parts.words().iter().fold(0u64, |s, &p| s.wrapping_add(p)))
    }
}
