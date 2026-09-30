//! hashbuild, Rust fold/merge twin: 2^S keys into a chained table of 2^S
//! slots with no lock. rayon `fold` builds a table (slot -> chain of
//! distinct keys) per piece of the index range, and `reduce` merges two
//! tables by taking the union of the chains slot by slot (the smaller
//! table into the larger).
use lockless::{mix, prng};
use rayon::prelude::*;
use std::collections::HashMap;

const LOG: usize = 20; // SIZE

type Table = HashMap<u32, Vec<u32>>;

fn key(i: u32) -> u32 {
    prng((i.wrapping_add(1)).wrapping_mul(2654435761) ^ 1779033703) & ((1u32 << (LOG + 4)) - 1)
}

fn slot(k: u32) -> u32 {
    k.wrapping_mul(2654435761) >> (32 - LOG)
}

fn insert(t: &mut Table, k: u32) {
    let chain = t.entry(slot(k)).or_default();
    if !chain.contains(&k) {
        chain.push(k);
    }
}

fn main() {
    let n = 1u32 << LOG;
    let table = (0..n)
        .into_par_iter()
        .fold(Table::new, |mut t, i| {
            insert(&mut t, key(i));
            t
        })
        .reduce(Table::new, |a, b| {
            let (mut big, small) = if a.len() >= b.len() { (a, b) } else { (b, a) };
            for (s, chain) in small {
                let into = big.entry(s).or_default();
                for k in chain {
                    if !into.contains(&k) {
                        into.push(k);
                    }
                }
            }
            big
        });
    let (mut occ, mut h) = (0u32, 0u32);
    for (&s, chain) in &table {
        let sum = chain.iter().fold(0u32, |a, &k| a.wrapping_add(k));
        occ += 1;
        h = h.wrapping_add(mix(s, sum.wrapping_add((chain.len() as u32).wrapping_mul(2654435761))));
    }
    println!("({}, {})", occ, h);
}
