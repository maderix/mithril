//! histogram, Rust fold/merge twin: 2^S keys into 2^16 bins with no shared
//! counter. rayon `fold` counts each piece of the index range into its own
//! bins, and `reduce` adds the pieces' bins together, bin by bin.
use lockless::{mix, prng};
use rayon::prelude::*;

const LOG: usize = 24; // SIZE
const BINS: usize = 65536;

fn key(i: u32) -> usize {
    (prng((i.wrapping_add(1)).wrapping_mul(2654435761)) >> 16) as usize
}

fn main() {
    let bins = (0..1u32 << LOG)
        .into_par_iter()
        .fold(
            || vec![0u32; BINS],
            |mut b, i| {
                b[key(i)] += 1;
                b
            },
        )
        .reduce(
            || vec![0u32; BINS],
            |mut a, b| {
                for (x, y) in a.iter_mut().zip(&b) {
                    *x += y;
                }
                a
            },
        );
    let h = bins
        .iter()
        .enumerate()
        .fold(0u32, |h, (i, &c)| h.wrapping_add(mix(i as u32, c)));
    println!("{}", h);
}
