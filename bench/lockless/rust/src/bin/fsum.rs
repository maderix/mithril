//! fsum, Rust twin: the f32 sum of 2^S values with rayon's `sum`. Rayon
//! splits the range adaptively (work stealing), so the grouping of the
//! additions, and the printed bits, can change with the thread count and
//! from run to run. Prints the sum's bit pattern.
use lockless::prng;
use rayon::prelude::*;

const LOG: usize = 24; // SIZE

fn val(i: u32) -> f32 {
    let h = prng((i.wrapping_add(1)).wrapping_mul(2654435761));
    (h & 16777215) as f32 / 16777216.0 - 0.5
}

fn main() {
    let s: f32 = (0..1u32 << LOG).into_par_iter().map(val).sum();
    println!("{}", s.to_bits());
}
