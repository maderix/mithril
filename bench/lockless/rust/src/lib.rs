//! Hashes shared by the Rust twins (the same as in each main.py and main.c).

/// xorshift: x ^= x << 13; x ^= x >> 17; x ^= x << 5.
#[inline]
pub fn prng(mut x: u32) -> u32 {
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

/// A position-weighted mix of entry i holding v.
#[inline]
pub fn mix(i: u32, v: u32) -> u32 {
    prng((i.wrapping_add(1)).wrapping_mul(2654435761) ^ v.wrapping_mul(2246822519))
}
