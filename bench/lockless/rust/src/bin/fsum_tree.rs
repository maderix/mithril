//! fsum, Rust fixed-shape tree twin: the same f32 sum as main.py, grouped
//! the same way. The range is cut into 2^P aligned chunks (P = min(S, 8));
//! rayon sums each chunk by the recursive halving tree and `collect`s the
//! partials in chunk order, then the partials are added by the same
//! halving tree. The grouping depends only on S, so the bits are the same
//! at every thread count and equal Mithril's: fsum.rs's schedule
//! dependence comes from rayon `sum`'s adaptive split, not from Rust.
use lockless::prng;
use rayon::prelude::*;

const LOG: usize = 24; // SIZE
const P: usize = if LOG < 8 { LOG } else { 8 };

fn val(i: u32) -> f32 {
    let h = prng((i.wrapping_add(1)).wrapping_mul(2654435761));
    (h & 16777215) as f32 / 16777216.0 - 0.5
}

/// The halving tree over 2^k values from lo (tsum in main.py).
fn tsum(lo: u32, k: usize) -> f32 {
    if k == 0 {
        return val(lo);
    }
    tsum(lo, k - 1) + tsum(lo + (1 << (k - 1)), k - 1)
}

/// The same tree over 2^k stored partials.
fn psum(a: &[f32]) -> f32 {
    if a.len() == 1 {
        return a[0];
    }
    let (l, r) = a.split_at(a.len() / 2);
    psum(l) + psum(r)
}

fn main() {
    let part: Vec<f32> = (0..1u32 << P).into_par_iter().map(|c| tsum(c << (LOG - P), LOG - P)).collect();
    println!("{}", psum(&part).to_bits());
}
