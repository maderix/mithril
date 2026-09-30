//! hashbuild, Rust twin: 2^S keys into a shared chained table of 2^S
//! slots, one `Mutex` per slot; an insert locks the slot and pushes the key
//! if absent (rayon over the index range).
use lockless::{mix, prng};
use rayon::prelude::*;
use std::sync::Mutex;

const LOG: usize = 20; // SIZE

fn key(i: u32) -> u32 {
    prng((i.wrapping_add(1)).wrapping_mul(2654435761) ^ 1779033703) & ((1u32 << (LOG + 4)) - 1)
}

fn slot(k: u32) -> usize {
    (k.wrapping_mul(2654435761) >> (32 - LOG)) as usize
}

fn main() {
    let n = 1usize << LOG;
    let table: Vec<Mutex<Vec<u32>>> = (0..n).map(|_| Mutex::new(Vec::new())).collect();
    (0..n as u32).into_par_iter().for_each(|i| {
        let k = key(i);
        let mut chain = table[slot(k)].lock().unwrap();
        if !chain.contains(&k) {
            chain.push(k);
        }
    });
    let (mut occ, mut h) = (0u32, 0u32);
    for (s, m) in table.into_iter().enumerate() {
        let chain = m.into_inner().unwrap();
        if chain.is_empty() {
            continue;
        }
        let sum = chain.iter().fold(0u32, |a, &k| a.wrapping_add(k));
        occ += 1;
        h = h.wrapping_add(mix(s as u32, sum.wrapping_add((chain.len() as u32).wrapping_mul(2654435761))));
    }
    println!("({}, {})", occ, h);
}
