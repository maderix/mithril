//! Range requests on the GPU: a proven fold over `[lo, hi)` (a fill of an
//! array, or a wrapping sum) runs one index per thread through the
//! program's leaves (`msl/range.metal`, `mithril_codegen::cprint::msl_leaves`).
//!
//! The arrays a request passes are copied into one shared buffer and reach
//! the leaves as device addresses; after the dispatch a fill's array is
//! copied back. Recursion in the leaves runs to a nesting limit fixed when
//! the leaves compile (`DEEP_LIMIT`): a leaf that reaches it faults, and the
//! leaves compile again with twice the limit. A leaf that faults past the
//! deepest limit, or indexes an array out of bounds, makes
//! [`RangeExec::run`] return `None`: the caller runs the request on the
//! CPU, whose result is the same by construction and which reports a fault.

use crate::metal::{Device, Dispatch, Pipeline};
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
                         device metal::atomic_uint *flags [[buffer(2)]], device uint *marked [[buffer(3)]],
                         uint t [[thread_position_in_grid]], uint lt [[thread_position_in_threadgroup]],
                         uint g [[threadgroup_position_in_grid]], uint size [[threads_per_threadgroup]]) {
  threadgroup ulong part[1024];
  // req: fid, lo, n, then the fold's ports from index 1 (args[k] =
  // req[3 + k]; ports 0 and 1 are the range); flags[0]: a depth fault,
  // flags[1]: how many indices the fast pass marked for the exact pass
  ulong v = 0;
#ifdef F32_EXACT
  bool active = t < metal::atomic_load_explicit(&flags[1], metal::memory_order_relaxed);
  uint k = active ? marked[t] : 0;
#else
  bool active = t < req[2];
  uint k = t;
#endif
  if (active) {
    i64 fuel[2] = {(i64)1 << 60, 0};
    v = (ulong)prog_range_leaf((uint)req[0], (i64)req[1] + (i64)k, req + 3, fuel);
    if (fuel[1] & DEEP_FAULT) {
      atomic_store_explicit(&flags[0], 1u, metal::memory_order_relaxed);
#ifndef F32_EXACT
    } else if (fuel[1] & F32_SUSPECT) {
      // the exact pass gives this index's term (and writes its element again)
      v = 0;
      marked[atomic_fetch_add_explicit(&flags[1], 1u, metal::memory_order_relaxed)] = k;
#endif
    }
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
/// The nesting limit the leaves first compile with, and the deepest.
const START_DEEP: usize = 32;
const MAX_DEEP: usize = 256;
/// The most call frames a Metal pipeline may declare.
const MAX_STACK: usize = 4096;

/// The GPU and the pipeline of one program's range leaves.
pub struct RangeExec {
    dev: Device,
    /// the leaves' source without its nesting limit
    src: String,
    /// call frames per nesting level (for calls the compiler keeps)
    frames: usize,
    /// the nesting limit compiled now and its passes
    current: Mutex<Passes>,
}

/// The leaves at one nesting limit: the fast pass, and the exact pass
/// (compiled when first needed), each with its threadgroup size.
struct Passes {
    deep: usize,
    fast: (Pipeline, usize),
    exact: Option<(Pipeline, usize)>,
}

enum Outcome {
    Done(u64),
    /// a leaf nested deeper than the limit
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
        let fast = Self::build(&dev, &src, frames, START_DEEP, false)?;
        Ok(RangeExec { dev, src, frames, current: Mutex::new(Passes { deep: START_DEEP, fast, exact: None }) })
    }

    /// The pipeline of the leaves at nesting limit `deep`, fast or exact.
    fn build(dev: &Device, src: &str, frames: usize, deep: usize, exact: bool) -> Result<(Pipeline, usize), String> {
        let t0 = std::time::Instant::now();
        let mode = if exact { "#define F32_EXACT\n" } else { "" };
        let lib = dev.compile(&format!("{mode}#define DEEP_LIMIT {deep}\n{src}"))
            // the compiler's report: its errors (generated code leaves unused locals)
            .map_err(|e| e.lines().filter(|l| l.contains("error")).collect::<Vec<_>>().join("\n"))?;
        trace(&format!("compiled the leaves at nesting {deep} in {} ms", t0.elapsed().as_millis()));
        // a level of guarded recursion is `frames` calls at most (+ the
        // kernel, prog_range_leaf and the scalar operations' fallbacks)
        let t1 = std::time::Instant::now();
        let pso = dev.pipeline(&lib, "range_launch", ((deep + 1) * frames + 32).min(MAX_STACK))?;
        trace(&format!("built the pipeline in {} ms", t1.elapsed().as_millis()));
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
        let mut cur = self.current.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            match self.attempt(&mut cur, fid, lo, n, args) {
                Outcome::Done(s) => return Some(s),
                Outcome::Deep if cur.deep < MAX_DEEP => {
                    let deep = cur.deep * 2;
                    let fast = Self::build(&self.dev, &self.src, self.frames, deep, false).ok()?;
                    *cur = Passes { deep, fast, exact: None };
                }
                Outcome::Deep | Outcome::Fault => return None,
            }
        }
    }

    fn attempt(&self, cur: &mut Passes, fid: u32, lo: i64, n: usize, args: &mut [Arg]) -> Outcome {
        let deep = cur.deep;
        let (pso, group) = (&cur.fast.0, cur.fast.1);
        let words: usize = args.iter().map(|a| if let Arg::Array { block, .. } = a { block.len() } else { 0 }).sum();
        let mut arrays = self.dev.buffer(words * 8);
        let base = arrays.gpu_address();
        let mut req = self.dev.buffer((5 + args.len()) * 8);
        let r = req.words_mut();
        r[..5].copy_from_slice(&[fid as u64, lo as u64, n as u64, 0, 0]);
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
        let flags = self.dev.buffer(8);
        let marked = self.dev.buffer(n * 4);
        let d = Dispatch { pipeline: pso, buffers: vec![&req, &parts, &flags, &marked], reached: vec![&arrays], threads: groups * group, group };
        let t0 = std::time::Instant::now();
        let ran = self.dev.run(&[d]);
        trace(&format!("{n} indices at nesting {deep}: {} ms", t0.elapsed().as_millis()));
        if let Err(e) = ran {
            trace(&e);
            return Outcome::Fault;
        }
        // flags: the depth fault in the low word, the marked count in the high
        let deep_fault = |flags: &crate::metal::Buffer| flags.words()[0] as u32 != 0;
        if deep_fault(&flags) {
            trace(&format!("a leaf nested deeper than {deep}"));
            return Outcome::Deep;
        }
        let mut sum = parts.words().iter().fold(0u64, |s, &p| s.wrapping_add(p));
        // the marked indices again, with exact binary32
        let count = (flags.words()[0] >> 32) as usize;
        if count > 0 {
            if cur.exact.is_none() {
                match Self::build(&self.dev, &self.src, self.frames, deep, true) {
                    Ok(p) => cur.exact = Some(p),
                    Err(e) => {
                        trace(&e);
                        return Outcome::Fault;
                    }
                }
            }
            let (pso, group) = cur.exact.as_ref().map(|(p, g)| (p, *g)).unwrap();
            let groups = count.div_ceil(group);
            let again = self.dev.buffer(groups * 8);
            let d = Dispatch { pipeline: pso, buffers: vec![&req, &again, &flags, &marked], reached: vec![&arrays], threads: groups * group, group };
            let t0 = std::time::Instant::now();
            if let Err(e) = self.dev.run(&[d]) {
                trace(&e);
                return Outcome::Fault;
            }
            trace(&format!("{count} indices again, exact: {} ms", t0.elapsed().as_millis()));
            if deep_fault(&flags) {
                return Outcome::Deep;
            }
            sum = again.words().iter().fold(sum, |s, &p| s.wrapping_add(p));
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
        Outcome::Done(sum)
    }
}
