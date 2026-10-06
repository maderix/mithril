//! Range requests on the Metal GPU (macOS, `--metal`): the GPU works on a
//! range request beside the CPU workers. In a range wave
//! (`engine::drain_ranges_gpu`) the coordinator submits a block of indices
//! to the GPU, runs CPU blocks while it is in flight, and accounts for it
//! when it completes; a block the GPU declines (a fault, no device) runs on
//! the CPU. Both give the same result: the leaves are the same LIR, the
//! device's scalar operations are bit-equal to the CPU's, a fill's indices
//! are disjoint and a sum's wrapping add does not depend on order.
//!
//! How much the GPU takes is a cost model from measurements: each launch
//! costs a fixed latency (submission and completion) plus a time per
//! index, and a CPU thread a time per index; the GPU takes a block only
//! when it finishes no later than the CPU threads would finish the rest
//! (`gpu_share`).

use crate::prelude::{arr_block, arr_len_of, as_i, tag, T_ARR};
use crate::{Program, RangeReq};
use mithril_metal::range::{Arg, Launch, RangeExec};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static ON: AtomicBool = AtomicBool::new(false);
static EXEC: OnceLock<Option<RangeExec>> = OnceLock::new();
/// The leaves' compile has started (on its own thread).
static COMPILING: std::sync::Once = std::sync::Once::new();

/// `MITHRIL_METAL_TEST=1`: the GPU takes part whatever it costs (the compile
/// is waited for, and every wave gives the GPU a share), so that tests run
/// the GPU's code.
fn testing() -> bool {
    static T: OnceLock<bool> = OnceLock::new();
    *T.get_or_init(|| std::env::var("MITHRIL_METAL_TEST").as_deref() == Ok("1"))
}

/// Send range requests to the GPU from now on.
pub fn enable() {
    ON.store(true, Ordering::Relaxed);
}

pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// The GPU, once its leaves are compiled. The first call starts the compile
/// on its own thread and the CPU runs requests meanwhile: a compile can take
/// longer than a program's whole run.
fn exec(prog: &dyn Program) -> Option<&'static RangeExec> {
    let leaves = prog.metal_leaves();
    COMPILING.call_once(|| {
        std::thread::spawn(move || {
            let e = RangeExec::new(leaves).map_err(|err| eprintln!("mithril: range launches run on the CPU ({err})")).ok();
            let _ = EXEC.set(e);
        });
    });
    if testing() {
        return EXEC.wait().as_ref();
    }
    EXEC.get()?.as_ref()
}

/// The GPU can run fold `fid`'s requests now.
pub fn ready(prog: &dyn Program, fid: u32) -> bool {
    on() && !prog.range_shape(fid).is_empty() && exec(prog).is_some()
}

/// Start compiling the leaves for the GPU, so they are ready by the first
/// range request.
pub fn prepare(prog: &dyn Program) {
    if on() {
        exec(prog);
    }
}

/// What one fold's work costs, measured (nanoseconds; 0: not yet).
#[derive(Clone, Copy, Default)]
struct Rates {
    /// a CPU thread, per index
    cpu: f64,
    /// a GPU launch: fixed latency, and per index
    latency: f64,
    gpu: f64,
}

static RATES: OnceLock<Mutex<HashMap<u32, Rates>>> = OnceLock::new();

fn rates(fid: u32) -> Rates {
    RATES.get_or_init(Default::default).lock().map(|m| m.get(&fid).copied().unwrap_or_default()).unwrap_or_default()
}

fn update(fid: u32, f: impl FnOnce(&mut Rates)) {
    if let Ok(mut m) = RATES.get_or_init(Default::default).lock() {
        f(m.entry(fid).or_default());
    }
}

/// A moving average that starts at the first sample.
fn ewma(old: f64, new: f64) -> f64 {
    if old == 0.0 { new } else { 0.75 * old + 0.25 * new }
}

/// A CPU thread ran `n` indices of fold `fid` in `ns`.
pub fn cpu_ran(fid: u32, n: u64, ns: f64) {
    if n > 0 {
        update(fid, |r| r.cpu = ewma(r.cpu, ns / n as f64));
    }
}

/// How many of the `rest` indices of fold `fid` the GPU should take next,
/// with `threads` CPU threads on the rest; 0 when the GPU would finish
/// after them. The GPU finishing a block of `b` at `latency + b * gpu`
/// while the threads finish the remainder at `(rest - b) * cpu / threads`
/// gives `b = (rest * cpu / threads - latency) / (gpu + cpu / threads)`.
/// Before the GPU's rates are known it takes a probe of two CPU blocks.
pub fn gpu_share(fid: u32, rest: u64, threads: usize, block: u64) -> u64 {
    if testing() {
        return block.max(rest / 2).min(rest);
    }
    let r = rates(fid);
    if r.cpu == 0.0 {
        return 0;
    }
    let per = r.cpu / threads as f64;
    let b = if r.gpu == 0.0 {
        (2 * block).min(rest / (threads as u64 + 1))
    } else {
        (((rest as f64 * per - r.latency) / (r.gpu + per)).max(0.0)) as u64
    };
    if b < block { 0 } else { b.min(rest) }
}

/// A block of a request in flight on the GPU.
pub struct Chunk {
    launch: Launch<'static>,
    args: Vec<Arg<'static>>,
    fid: u32,
    n: u64,
}

impl Chunk {
    pub fn done(&self) -> bool {
        self.launch.done()
    }

    /// Its sum (a fill's is meaningless), or `None` to run it on the CPU.
    pub fn finish(mut self) -> Option<u64> {
        let (sum, t) = self.launch.finish(&mut self.args)?;
        update(self.fid, |r| {
            r.latency = ewma(r.latency, (t.wall_ns - t.gpu_ns).max(0.0));
            r.gpu = ewma(r.gpu, t.gpu_ns / self.n as f64);
        });
        Some(sum)
    }
}

/// Start indices `[lo, hi)` of request `r` on the GPU (`None`: it cannot).
pub fn start(prog: &dyn Program, r: &RangeReq, lo: i64, hi: i64) -> Option<Chunk> {
    let exec = exec(prog)?;
    let shape = prog.range_shape(r.fid);
    if shape.len() != r.ports.len() {
        return None;
    }
    let mut blocks: Vec<*mut u64> = Vec::new();
    let mut args = Vec::with_capacity(shape.len());
    for (k, &how) in shape.iter().enumerate().skip(2) {
        let p = r.ports[k];
        args.push(match how {
            1 => Arg::Int(as_i(p)),
            3 => Arg::Int(0),
            2 | 4 if tag(p) == T_ARR => {
                let b = arr_block(p);
                // one array passed twice would be two mutable views of it
                if blocks.contains(&b) {
                    return None;
                }
                blocks.push(b);
                // SAFETY: a live array port's block holds its length + 2
                // words, and the request owns or borrows it until it
                // completes, after every chunk of it has finished. A read
                // array is not written meanwhile; a fill's elements are
                // written only at the GPU's indices, which no CPU block has.
                let block = unsafe { std::slice::from_raw_parts_mut(b, arr_len_of(p) + 2) };
                Arg::Array { block, write: how == 4 }
            }
            _ => return None,
        });
    }
    let launch = exec.start(r.fid, lo, hi, &mut args)?;
    if std::env::var_os("MITHRIL_METAL_TRACE").is_some() {
        eprintln!("metal: fold {} [{lo}, {hi}) to the GPU", r.fid);
    }
    Some(Chunk { launch, args, fid: r.fid, n: (hi - lo) as u64 })
}
