//! components, Rust twin: lock-free union-find with `compare_exchange`. A
//! union links the larger root under the smaller one (retrying if another
//! thread linked it first); find halves paths with a CAS. Every root is its
//! component's minimum id.
use lockless::{mix, prng};
use rayon::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering::SeqCst};

const LOG: usize = 20; // SIZE

fn end(x: u32) -> u32 {
    prng((x.wrapping_add(1)).wrapping_mul(2654435761)) & ((1u32 << LOG) - 1)
}

fn find(parent: &[AtomicU32], mut x: u32) -> u32 {
    loop {
        let p = parent[x as usize].load(SeqCst);
        if p == x {
            return x;
        }
        let g = parent[p as usize].load(SeqCst);
        if g != p {
            let _ = parent[x as usize].compare_exchange_weak(p, g, SeqCst, SeqCst);
        }
        x = p;
    }
}

fn unite(parent: &[AtomicU32], a: u32, b: u32) {
    loop {
        let (a, b) = (find(parent, a), find(parent, b));
        if a == b {
            return;
        }
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        if parent[hi as usize].compare_exchange(hi, lo, SeqCst, SeqCst).is_ok() {
            return;
        }
    }
}

fn main() {
    let n = 1u32 << LOG;
    let parent: Vec<AtomicU32> = (0..n).map(AtomicU32::new).collect();
    (0..n).into_par_iter().for_each(|e| unite(&parent, end(2 * e), end(2 * e + 1)));
    let (mut comps, mut h) = (0u32, 0u32);
    for v in 0..n {
        let r = find(&parent, v);
        comps += (r == v) as u32;
        h = h.wrapping_add(mix(v, r));
    }
    println!("({}, {})", comps, h);
}
