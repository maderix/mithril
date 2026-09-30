//! histogram, Rust twin: 2^S keys into 2^16 shared atomic bins
//! (`fetch_add` per key, rayon over the index range).
use lockless::{mix, prng};
use rayon::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

const LOG: usize = 24; // SIZE
const BINS: usize = 65536;

fn key(i: u32) -> usize {
    (prng((i.wrapping_add(1)).wrapping_mul(2654435761)) >> 16) as usize
}

fn main() {
    let bins: Vec<AtomicU32> = (0..BINS).map(|_| AtomicU32::new(0)).collect();
    (0..1u32 << LOG).into_par_iter().for_each(|i| {
        bins[key(i)].fetch_add(1, Ordering::Relaxed);
    });
    let h = bins
        .iter()
        .enumerate()
        .fold(0u32, |h, (i, b)| h.wrapping_add(mix(i as u32, b.load(Ordering::Relaxed))));
    println!("{}", h);
}
