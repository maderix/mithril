//! Range requests on the Metal GPU (macOS, `--metal`): a request whose
//! fold has leaves the GPU runs goes to `mithril_metal::range`; any other,
//! and any the GPU declines (a fault, no device), runs on the CPU as
//! before. Both give the same result: the leaves are the same LIR, and the
//! device's scalar operations are bit-equal to the CPU's.

use crate::prelude::{arr_block, arr_len_of, as_i, tag, T_ARR};
use crate::{Program, RangeReq};
use mithril_metal::range::{Arg, RangeExec};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static ON: AtomicBool = AtomicBool::new(false);
static EXEC: OnceLock<Option<RangeExec>> = OnceLock::new();

/// Send range requests to the GPU from now on.
pub fn enable() {
    ON.store(true, Ordering::Relaxed);
}

pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

fn exec(prog: &dyn Program) -> Option<&'static RangeExec> {
    EXEC.get_or_init(|| match RangeExec::new(prog.metal_leaves()) {
        Ok(e) => Some(e),
        Err(err) => {
            eprintln!("mithril: range launches run on the CPU ({err})");
            None
        }
    })
    .as_ref()
}

/// Request `r` on the GPU: its sum (meaningless for a fill, whose array is
/// written in place), or `None` to run it on the CPU.
pub fn run(prog: &dyn Program, r: &RangeReq) -> Option<u64> {
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
                // SAFETY: a live array port's block holds its length + 2 words,
                // and the request owns or borrows it until it completes
                let block = unsafe { std::slice::from_raw_parts_mut(b, arr_len_of(p) + 2) };
                Arg::Array { block, write: how == 4 }
            }
            _ => return None,
        });
    }
    let res = exec.run(r.fid, r.lo, r.hi, &mut args);
    if std::env::var_os("MITHRIL_METAL_TRACE").is_some() {
        eprintln!("metal: fold {} [{}, {}) shape {:?} -> {:?}", r.fid, r.lo, r.hi, shape, res);
    }
    res
}
