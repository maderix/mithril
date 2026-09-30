//! bfs, Rust twin: level-synchronous BFS from node 0; a node is claimed for
//! the next level by `compare_exchange` on its atomic level word, and the
//! next frontier is the claimed nodes collected by rayon.
use lockless::prng;
use rayon::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

const LOG: usize = 20; // SIZE
const UNSEEN: u32 = u32::MAX;

fn nbr(v: u32, j: u32) -> u32 {
    prng((v.wrapping_mul(8).wrapping_add(j + 1)).wrapping_mul(2654435761) ^ 2246822519) & ((1u32 << LOG) - 1)
}

fn main() {
    let n = 1usize << LOG;
    let level: Vec<AtomicU32> = (0..n).map(|_| AtomicU32::new(UNSEEN)).collect();
    level[0].store(0, Ordering::Relaxed);
    let mut front = vec![0u32];
    let (mut d, mut reached, mut acc) = (0u32, 0u32, 0u64);
    while !front.is_empty() {
        reached += front.len() as u32;
        acc += d as u64 * front.len() as u64;
        let level = &level;
        front = front
            .par_iter()
            .flat_map_iter(|&v| {
                (0..8).map(move |j| nbr(v, j)).filter(move |&u| {
                    let a = &level[u as usize];
                    a.load(Ordering::Relaxed) == UNSEEN
                        && a.compare_exchange(UNSEEN, d + 1, Ordering::Relaxed, Ordering::Relaxed).is_ok()
                })
            })
            .collect();
        d += 1;
    }
    println!("({}, {})", reached, acc);
}
