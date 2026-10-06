//! Range requests on the GPU: a proven fold over `[lo, hi)` (a fill of an
//! array, or a wrapping sum) runs one index per thread through the
//! program's leaves (`msl/range.metal`, `mithril_codegen::cprint::msl_leaves`).
//!
//! The arrays a request reads are copied into one shared buffer and reach
//! the leaves as device addresses. A fill never reads its array, so its
//! copy carries no contents, only a write map: each element the GPU writes
//! marks a byte, and those elements alone are copied back. The CPU may
//! write other elements of the same array meanwhile (a fill's indices are
//! disjoint), so a launch can take part of a request while the CPU runs
//! the rest.
//!
//! A launch starts without waiting ([`RangeExec::start`]); the caller does
//! other work and completes it later ([`Launch::finish`]). Recursion in the
//! leaves runs to a nesting limit fixed when the leaves compile
//! (`DEEP_LIMIT`): a leaf that reaches it faults, and the leaves compile
//! again with twice the limit. A launch that faults past the deepest limit,
//! or indexes an array out of bounds, finishes with `None`: the caller runs
//! its range on the CPU, whose result is the same by construction and which
//! reports a fault.

use crate::metal::{Buffer, Device, Dispatch, Pipeline, Submitted, Timing};
use std::sync::{Mutex, MutexGuard};

/// One argument of a request, in port order from index 2 (the fold's index
/// and bound are the range itself).
pub enum Arg<'a> {
    Int(i64),
    /// an int array's block (`[refcount, length | flags, elements...]`):
    /// read (copied in), or a fill's (`write`: the elements the GPU writes
    /// are copied back)
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
/// A staged block's length word carries this when a write map follows its
/// elements (range.metal `ARR_MAP`).
const ARR_MAP: u64 = 1 << 61;
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
    /// the nesting limit compiled now, its passes and the kept buffers
    current: Mutex<Passes>,
}

/// The leaves at one nesting limit: the fast pass, and the exact pass
/// (compiled when first needed), each with its threadgroup size.
struct Passes {
    deep: usize,
    fast: (Pipeline, usize),
    exact: Option<(Pipeline, usize)>,
    /// buffers kept between launches: a fresh buffer costs the GPU its
    /// page mappings on first touch
    kept: Kept,
}

/// The buffers of the last launches, by role; each grows when a launch
/// needs more and is otherwise reused.
#[derive(Default)]
struct Kept {
    arrays: Option<Buffer>,
    req: Option<Buffer>,
    parts: Option<Buffer>,
    again: Option<Buffer>,
    flags: Option<Buffer>,
    marked: Option<Buffer>,
}

/// `slot`'s buffer, of at least `bytes` (grown by doubling).
fn kept<'a>(dev: &Device, slot: &'a mut Option<Buffer>, bytes: usize) -> &'a mut Buffer {
    if slot.as_ref().map_or(true, |b| b.bytes() < bytes) {
        let cap = slot.as_ref().map_or(0, |b| b.bytes()) * 2;
        *slot = Some(dev.buffer(bytes.max(cap).max(4096)));
    }
    slot.as_mut().unwrap()
}

/// The words a staged argument takes: a read block whole, a fill's header
/// and elements followed by its write map (one byte per element).
fn staged_words(a: &Arg) -> usize {
    match a {
        Arg::Int(_) => 0,
        Arg::Array { block, write: false } => block.len(),
        Arg::Array { block, write: true } => block.len() + (block.len() - 2).div_ceil(8),
    }
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

/// A launch on its way through the GPU; [`Launch::finish`] completes it.
pub struct Launch<'a> {
    exec: &'a RangeExec,
    cur: MutexGuard<'a, Passes>,
    sub: Submitted,
    fid: u32,
    lo: i64,
    n: usize,
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
        Ok(RangeExec { dev, src, frames, current: Mutex::new(Passes { deep: START_DEEP, fast, exact: None, kept: Kept::default() }) })
    }

    /// The pipeline of the leaves at nesting limit `deep`, fast or exact.
    fn build(dev: &Device, src: &str, frames: usize, deep: usize, exact: bool) -> Result<(Pipeline, usize), String> {
        let t0 = std::time::Instant::now();
        let mode = if exact { "#define F32_EXACT\n" } else { "" };
        let lib = dev.compile(&format!("{mode}#define DEEP_LIMIT {deep}\n{src}"))
            // the compiler's report: its errors (generated code leaves unused locals)
            .map_err(|e| e.lines().filter(|l| l.contains("error")).collect::<Vec<_>>().join("\n"))?;
        // a level of guarded recursion is `frames` calls at most (+ the
        // kernel, prog_range_leaf and the scalar operations' fallbacks)
        let pso = dev.pipeline(&lib, "range_launch", ((deep + 1) * frames + 32).min(MAX_STACK))?;
        trace(&format!("built the leaves at nesting {deep} in {} ms", t0.elapsed().as_millis()));
        // the reduction halves the group: a power of two
        let group = 1 << pso.max_group().min(1024).ilog2();
        Ok((pso, group))
    }

    /// Fold `fid` over `[lo, hi)` and wait: the wrapping sum of the leaves
    /// (a fill's is meaningless), or `None` to run it on the CPU.
    pub fn run(&self, fid: u32, lo: i64, hi: i64, args: &mut [Arg]) -> Option<u64> {
        self.start(fid, lo, hi, args)?.finish(args).map(|(s, _)| s)
    }

    /// Start fold `fid` over `[lo, hi)` without waiting (`None`: the range
    /// is empty or too long for one launch).
    pub fn start(&self, fid: u32, lo: i64, hi: i64, args: &mut [Arg]) -> Option<Launch<'_>> {
        let n = (hi - lo).max(0) as usize;
        if n == 0 || n > u32::MAX as usize {
            return None;
        }
        let mut cur = self.current.lock().unwrap_or_else(|p| p.into_inner());
        let sub = self.submit(&mut cur, fid, lo, n, args, false);
        Some(Launch { exec: self, cur, sub, fid, lo, n })
    }

    /// Stage the arguments (fast pass) and start a pass over `n` indices,
    /// or over the indices the fast pass marked (`exact`).
    fn submit(&self, cur: &mut Passes, fid: u32, lo: i64, n: usize, args: &mut [Arg], exact: bool) -> Submitted {
        let (pso, group) = if exact { cur.exact.as_ref().map(|(p, g)| (p, *g)).unwrap() } else { (&cur.fast.0, cur.fast.1) };
        let k = &mut cur.kept;
        if !exact {
            let words: usize = args.iter().map(staged_words).sum();
            let arrays = kept(&self.dev, &mut k.arrays, words * 8);
            let base = arrays.gpu_address();
            let staged = arrays.words_mut();
            let req = kept(&self.dev, &mut k.req, (5 + args.len()) * 8).words_mut();
            req[..5].copy_from_slice(&[fid as u64, lo as u64, n as u64, 0, 0]);
            let mut at = 0;
            for (i, a) in args.iter().enumerate() {
                req[5 + i] = match a {
                    Arg::Int(v) => *v as u64,
                    Arg::Array { block, write } => {
                        let len = block.len();
                        if *write {
                            // no contents (a fill never reads its array); the
                            // write map after the elements starts clear
                            staged[at + 1] = block[1] | ARR_MAP;
                            staged[at + len..at + staged_words(a)].fill(0);
                        } else {
                            staged[at..at + len].copy_from_slice(block);
                        }
                        // the refcount word becomes the fault mark
                        staged[at] = 0;
                        let handle = (T_ARR << 56) | (base + at as u64 * 8);
                        at += staged_words(a);
                        handle
                    }
                };
            }
            kept(&self.dev, &mut k.flags, 8).words_mut()[0] = 0;
            kept(&self.dev, &mut k.marked, n * 4);
            kept(&self.dev, &mut k.parts, n.div_ceil(group) * 8);
            kept(&self.dev, &mut k.again, n.div_ceil(group) * 8);
        }
        let count = if exact { (k.flags.as_ref().unwrap().words()[0] >> 32) as usize } else { n };
        let groups = count.div_ceil(group);
        let (arrays, req, flags, marked) = (k.arrays.as_ref().unwrap(), k.req.as_ref().unwrap(), k.flags.as_ref().unwrap(), k.marked.as_ref().unwrap());
        let parts = if exact { k.again.as_ref().unwrap() } else { k.parts.as_ref().unwrap() };
        self.dev.submit(&[Dispatch { pipeline: pso, buffers: vec![req, parts, flags, marked], reached: vec![arrays], threads: groups * group, group }])
    }

    /// Wait for the submitted fast pass, then (synchronously) the exact
    /// pass on the indices it marked.
    fn complete(&self, cur: &mut Passes, sub: Submitted, fid: u32, lo: i64, n: usize, args: &mut [Arg]) -> (Outcome, Timing) {
        let deep = cur.deep;
        let timing = match sub.wait() {
            Ok(t) => t,
            Err(e) => {
                trace(&e);
                return (Outcome::Fault, Timing::default());
            }
        };
        trace(&format!("{n} indices at nesting {deep}: {:.2} ms ({:.2} ms on the GPU)", timing.wall_ns / 1e6, timing.gpu_ns / 1e6));
        let k = &cur.kept;
        // flags: the depth fault in the low word, the marked count in the high
        let flags = k.flags.as_ref().unwrap().words()[0];
        if flags as u32 != 0 {
            trace(&format!("a leaf nested deeper than {deep}"));
            return (Outcome::Deep, timing);
        }
        let groups = n.div_ceil(cur.fast.1);
        let mut sum = k.parts.as_ref().unwrap().words()[..groups].iter().fold(0u64, |s, &p| s.wrapping_add(p));
        let count = (flags >> 32) as usize;
        if count > 0 {
            // the marked indices again, with exact binary32
            if cur.exact.is_none() {
                match Self::build(&self.dev, &self.src, self.frames, deep, true) {
                    Ok(p) => cur.exact = Some(p),
                    Err(e) => {
                        trace(&e);
                        return (Outcome::Fault, timing);
                    }
                }
            }
            match self.submit(cur, fid, lo, n, args, true).wait() {
                Ok(_) => {}
                Err(e) => {
                    trace(&e);
                    return (Outcome::Fault, timing);
                }
            }
            trace(&format!("{count} indices again, exact"));
            let k = &cur.kept;
            if k.flags.as_ref().unwrap().words()[0] as u32 != 0 {
                return (Outcome::Deep, timing);
            }
            let groups = count.div_ceil(cur.exact.as_ref().unwrap().1);
            sum = k.again.as_ref().unwrap().words()[..groups].iter().fold(sum, |s, &p| s.wrapping_add(p));
        }
        let staged = cur.kept.arrays.as_ref().unwrap().words();
        let mut at = 0;
        for a in args.iter() {
            if let Arg::Array { .. } = a {
                if staged[at] != 0 {
                    trace("a leaf indexed an array out of bounds");
                    return (Outcome::Fault, timing);
                }
            }
            at += staged_words(a);
        }
        // the elements the GPU wrote, by their write map
        let mut at = 0;
        for a in args.iter_mut() {
            let words = staged_words(a);
            if let Arg::Array { block, write: true } = a {
                let len = block.len() - 2;
                let map = &staged[at + 2 + len..at + words];
                for (w, &bits) in map.iter().enumerate().filter(|(_, &b)| b != 0) {
                    for j in 0..8usize {
                        let e = w * 8 + j;
                        if e < len && (bits >> (8 * j)) & 0xff != 0 {
                            block[2 + e] = staged[at + 2 + e];
                        }
                    }
                }
            }
            at += words;
        }
        (Outcome::Done(sum), timing)
    }
}

impl Launch<'_> {
    /// The GPU has finished the fast pass (`finish` then waits no longer
    /// than an exact pass, when indices were marked).
    pub fn done(&self) -> bool {
        self.sub.done()
    }

    /// Complete the launch: its wrapping sum and what the fast pass cost,
    /// or `None` to run its range on the CPU.
    pub fn finish(self, args: &mut [Arg]) -> Option<(u64, Timing)> {
        let Launch { exec, mut cur, sub, fid, lo, n } = self;
        let mut sub = sub;
        loop {
            match exec.complete(&mut cur, sub, fid, lo, n, args) {
                (Outcome::Done(s), t) => return Some((s, t)),
                (Outcome::Deep, _) if cur.deep < MAX_DEEP => {
                    let deep = cur.deep * 2;
                    cur.fast = RangeExec::build(&exec.dev, &exec.src, exec.frames, deep, false).ok()?;
                    cur.deep = deep;
                    cur.exact = None;
                    sub = exec.submit(&mut cur, fid, lo, n, args, false);
                }
                _ => return None,
            }
        }
    }
}
