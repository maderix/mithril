//! Co-execution: several engines (the CPU workers, a GPU) share the chunks
//! of one proven fold.
//!
//! Each engine runs the same program. When a proven sum fold over a large
//! range is reached, the fold is a **job** keyed by its function, its range
//! and its scalar arguments. The key fixes the result, so any engine holding
//! a job with that key computes the same value. Engines claim chunks of the
//! range from the job's cursor, compute each chunk's partial (the fold over
//! the chunk from the identity) and store it in the chunk's slot. When the
//! cursor is past the end and every slot is filled, each engine adds the
//! partials. Wrapping integer addition is associative and commutative, so
//! the total does not depend on which engine ran which chunk, or in what
//! order.
//!
//! The table lives in a file every engine maps (`/dev/shm` or any path),
//! so engines may be threads or processes. Nothing waits on a lock: a claim
//! is one atomic add, a partial one store. An engine that never joins a job
//! leaves its share to the others; a chunk whose engine stops answering is
//! recomputed by the engine that waits for it (chunks are pure, so running
//! one twice gives the same partial).

use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Words of a job's key: function, range, kind, then up to 12 scalar
/// arguments.
pub const KEY_WORDS: usize = 16;
const JOBS: usize = 64;
const CHUNKS: usize = 1024;
const MAGIC: u64 = 0x4d49_5448_434f_4f50; // "MITHCOOP"

#[repr(C)]
struct Slot {
    lo: AtomicI64,
    hi: AtomicI64,
    /// 0 free, 1 claimed, 2 done
    state: AtomicU32,
    _pad: u32,
    value: AtomicU64,
}

#[repr(C)]
struct Entry {
    /// 0 empty, 1 being written, 2 ready
    state: AtomicU32,
    _pad: u32,
    key: [AtomicU64; KEY_WORDS],
    lo: AtomicI64,
    hi: AtomicI64,
    /// the next index to claim
    cursor: AtomicI64,
    /// chunks handed out so far
    chunks: AtomicU32,
    /// engines sharing the job (sizes the chunks)
    partners: AtomicU32,
    slots: [Slot; CHUNKS],
}

#[repr(C)]
struct Table {
    magic: AtomicU64,
    jobs: [Entry; JOBS],
}

/// A mapped co-execution table.
pub struct Coop {
    table: &'static Table,
}

static CHANNEL: OnceLock<Coop> = OnceLock::new();
/// When the channel opened (the clock of `trace`).
static T0: OnceLock<Instant> = OnceLock::new();

/// The process's co-execution table, once [`open`] has mapped one.
pub fn channel() -> Option<&'static Coop> {
    CHANNEL.get()
}

/// Map the table at `path` (created and zeroed if absent) as this process's
/// channel. Returns the channel already set, if any.
pub fn open(path: &std::path::Path) -> Result<&'static Coop, String> {
    if let Some(c) = CHANNEL.get() {
        return Ok(c);
    }
    let c = Coop::map(path)?;
    T0.get_or_init(Instant::now);
    Ok(CHANNEL.get_or_init(|| c))
}

extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
}
const PROT_RW: i32 = 3;
const MAP_SHARED: i32 = 1;

impl Coop {
    /// Map the table at `path`; a new file is sized and starts empty.
    pub fn map(path: &std::path::Path) -> Result<Coop, String> {
        use std::os::fd::AsRawFd;
        let err = |e: std::io::Error| format!("co-execution table {}: {e}", path.display());
        let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path).map_err(err)?;
        let len = std::mem::size_of::<Table>();
        if file.metadata().map_err(err)?.len() < len as u64 {
            file.set_len(len as u64).map_err(err)?;
        }
        // SAFETY: a shared mapping of a file of at least `len` bytes; the
        // table holds only atomics, so concurrent access from other mappings
        // is well defined. The mapping is never unmapped (it lives as long
        // as the process, like the channel).
        let p = unsafe { mmap(std::ptr::null_mut(), len, PROT_RW, MAP_SHARED, file.as_raw_fd(), 0) };
        if p as isize == -1 || p.is_null() {
            return Err(err(std::io::Error::last_os_error()));
        }
        let table = unsafe { &*(p as *const Table) };
        if table.magic.compare_exchange(0, MAGIC, Ordering::AcqRel, Ordering::Acquire).is_err_and(|m| m != MAGIC) {
            return Err(format!("co-execution table {}: not a table", path.display()));
        }
        Ok(Coop { table })
    }

    /// The job for `key` over `[lo, hi)`, created if no engine has it yet.
    /// `None` when the table is full (the caller runs the fold alone).
    pub fn job(&self, key: &[u64], lo: i64, hi: i64) -> Option<Job<'_>> {
        assert!(key.len() <= KEY_WORDS, "co-execution key too long");
        let mut k = [0u64; KEY_WORDS];
        k[..key.len()].copy_from_slice(key);
        let start = (fnv(&k) as usize) % JOBS;
        for i in 0..JOBS {
            let e = &self.table.jobs[(start + i) % JOBS];
            loop {
                match e.state.load(Ordering::Acquire) {
                    0 => {
                        if e.state.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                            for (w, v) in e.key.iter().zip(k) {
                                w.store(v, Ordering::Relaxed);
                            }
                            e.lo.store(lo, Ordering::Relaxed);
                            e.hi.store(hi, Ordering::Relaxed);
                            e.cursor.store(lo, Ordering::Relaxed);
                            e.state.store(2, Ordering::Release);
                            e.partners.fetch_add(1, Ordering::AcqRel);
                            return Some(Job { e });
                        }
                    }
                    1 => std::hint::spin_loop(),
                    _ => break,
                }
            }
            if e.key.iter().zip(k).all(|(w, v)| w.load(Ordering::Relaxed) == v) {
                e.partners.fetch_add(1, Ordering::AcqRel);
                return Some(Job { e });
            }
        }
        None
    }
}

/// One engine's handle on a shared job.
pub struct Job<'a> {
    e: &'a Entry,
}

/// A claimed chunk: slot `k` covers `[lo, hi)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chunk {
    pub k: usize,
    pub lo: i64,
    pub hi: i64,
}

impl Job<'_> {
    /// Claim the next chunk, or `None` once the range is handed out. Chunks
    /// shrink as the range runs out (a share of what is left per partner),
    /// so the last chunks are small and engines finish together.
    pub fn claim(&self) -> Option<Chunk> {
        let e = self.e;
        let hi = e.hi.load(Ordering::Relaxed);
        let len = hi - e.lo.load(Ordering::Relaxed);
        // at least 1 / CHUNKS of the range, so the slots never run out
        let min = (len / (CHUNKS as i64 / 2)).max(1);
        let ways = 4 * e.partners.load(Ordering::Acquire).max(1) as i64;
        let mut at = e.cursor.load(Ordering::Acquire);
        loop {
            if at >= hi {
                return None;
            }
            let end = (at + ((hi - at) / ways).max(min)).min(hi);
            match e.cursor.compare_exchange_weak(at, end, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => {
                    let k = e.chunks.fetch_add(1, Ordering::AcqRel) as usize;
                    assert!(k < CHUNKS, "co-execution: chunk slots exhausted");
                    let s = &e.slots[k];
                    s.lo.store(at, Ordering::Relaxed);
                    s.hi.store(end, Ordering::Relaxed);
                    s.state.store(1, Ordering::Release);
                    return Some(Chunk { k, lo: at, hi: end });
                }
                Err(now) => at = now,
            }
        }
    }

    /// Store chunk `k`'s partial.
    pub fn put(&self, k: usize, value: u64) {
        let s = &self.e.slots[k];
        s.value.store(value, Ordering::Relaxed);
        s.state.store(2, Ordering::Release);
    }

    /// The partials once every chunk is done, in chunk order. A chunk still
    /// open after `patience` is returned in `Err` for the caller to compute
    /// (and `put`); the caller then asks again.
    pub fn finish(&self, patience: Duration) -> Result<Vec<u64>, Vec<Chunk>> {
        let e = self.e;
        let t0 = Instant::now();
        loop {
            let n = e.chunks.load(Ordering::Acquire) as usize;
            // a claim between the cursor's move and its slot write: wait for it
            let covered: i64 = e.slots[..n].iter().filter(|s| s.state.load(Ordering::Acquire) != 0).map(|s| s.hi.load(Ordering::Relaxed) - s.lo.load(Ordering::Relaxed)).sum();
            let whole = covered == e.hi.load(Ordering::Relaxed) - e.lo.load(Ordering::Relaxed);
            let open: Vec<Chunk> = e.slots[..n]
                .iter()
                .enumerate()
                .filter(|(_, s)| s.state.load(Ordering::Acquire) != 2)
                .map(|(k, s)| Chunk { k, lo: s.lo.load(Ordering::Relaxed), hi: s.hi.load(Ordering::Relaxed) })
                .collect();
            if whole && open.is_empty() {
                return Ok(e.slots[..n].iter().map(|s| s.value.load(Ordering::Relaxed)).collect());
            }
            if t0.elapsed() > patience && whole {
                return Err(open);
            }
            std::thread::sleep(Duration::from_micros(200));
        }
    }
}

/// The total of a sum fold's partials: wrapping add, then the low 32 bits
/// for a fold mod 2^32 (`kind` 2), as a range request completes.
pub fn total(parts: &[u64], kind: u32) -> i64 {
    let s = parts.iter().fold(0u64, |a, &p| a.wrapping_add(p)) as i64;
    if kind == 2 {
        s & 0xffff_ffff
    } else {
        s
    }
}

/// `MITHRIL_COOP_STATS=1`: each engine reports every chunk it ran (which
/// engine, the slot, the range, when it started and how long it took,
/// seconds since the process started).
pub fn trace(engine: &str, c: Chunk, started: Instant, took: Duration) {
    static ON: OnceLock<bool> = OnceLock::new();
    let t0 = *T0.get_or_init(Instant::now);
    if *ON.get_or_init(|| std::env::var_os("MITHRIL_COOP_STATS").is_some()) {
        let at = started.saturating_duration_since(t0).as_secs_f64();
        eprintln!("mithril-coop: {engine} chunk {} [{}, {}) at {at:.3} s took {:.3} s", c.k, c.lo, c.hi, took.as_secs_f64());
    }
}

fn fnv(k: &[u64]) -> u64 {
    k.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &w| (h ^ w).wrapping_mul(0x100_0000_01b3))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("mithril-coop-{}-{name}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    /// The fold every test shares: the sum of a hash over an index range.
    fn leaf(i: i64) -> u64 {
        (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 7
    }

    fn chunk_sum(c: Chunk) -> u64 {
        (c.lo..c.hi).fold(0u64, |a, i| a.wrapping_add(leaf(i)))
    }

    #[test]
    fn partners_on_separate_mappings_cover_the_range_once_and_agree() {
        let path = table("partners");
        let (lo, hi) = (-1000i64, 1_000_003i64);
        let want = (lo..hi).fold(0u64, |a, i| a.wrapping_add(leaf(i))) as i64;
        let key = [7, lo as u64, hi as u64, 1, 42];
        // four engines, each with its own mapping of the file (as processes)
        let totals: Vec<(i64, usize)> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..4)
                .map(|_| {
                    let path = path.clone();
                    s.spawn(move || {
                        let c = Coop::map(&path).unwrap();
                        let job = c.job(&key, lo, hi).unwrap();
                        let mut mine = 0;
                        while let Some(ch) = job.claim() {
                            job.put(ch.k, chunk_sum(ch));
                            mine += 1;
                        }
                        (total(&job.finish(Duration::from_secs(30)).unwrap(), 1), mine)
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for (t, _) in &totals {
            assert_eq!(*t, want);
        }
        assert!(totals.iter().map(|t| t.1).sum::<usize>() <= CHUNKS);
        // chunks are disjoint and cover the range exactly
        let c = Coop::map(&path).unwrap();
        let job = c.job(&key, lo, hi).unwrap();
        let n = job.e.chunks.load(Ordering::Relaxed) as usize;
        let mut spans: Vec<(i64, i64)> = job.e.slots[..n].iter().map(|s| (s.lo.load(Ordering::Relaxed), s.hi.load(Ordering::Relaxed))).collect();
        spans.sort();
        assert_eq!(spans.first().unwrap().0, lo);
        assert_eq!(spans.last().unwrap().1, hi);
        assert!(spans.windows(2).all(|w| w[0].1 == w[1].0));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_lone_engine_takes_every_chunk() {
        let path = table("alone");
        let c = Coop::map(&path).unwrap();
        let job = c.job(&[1, 0, 100, 2], 0, 100).unwrap();
        while let Some(ch) = job.claim() {
            job.put(ch.k, 0xffff_ffff);
        }
        let parts = job.finish(Duration::from_millis(10)).unwrap();
        assert_eq!(total(&parts, 2), (0xffff_ffffu64.wrapping_mul(parts.len() as u64) & 0xffff_ffff) as i64);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_chunk_left_open_is_handed_back_for_recomputation() {
        let path = table("stall");
        let c = Coop::map(&path).unwrap();
        let job = c.job(&[3, 0, 64, 1], 0, 64).unwrap();
        let stalled = job.claim().unwrap(); // a partner that claimed and stopped
        while let Some(ch) = job.claim() {
            job.put(ch.k, chunk_sum(ch));
        }
        let open = job.finish(Duration::from_millis(20)).unwrap_err();
        assert_eq!(open, vec![stalled]);
        job.put(stalled.k, chunk_sum(stalled));
        let want = (0..64).fold(0u64, |a, i| a.wrapping_add(leaf(i))) as i64;
        assert_eq!(total(&job.finish(Duration::from_millis(20)).unwrap(), 1), want);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn different_keys_are_different_jobs_and_equal_keys_share() {
        let path = table("keys");
        let c = Coop::map(&path).unwrap();
        let a = c.job(&[1, 0, 10, 1, 5], 0, 10).unwrap();
        let b = c.job(&[1, 0, 10, 1, 6], 0, 10).unwrap();
        assert!(!std::ptr::eq(a.e, b.e));
        let a2 = Coop::map(&path).unwrap();
        assert!(std::ptr::eq(a2.job(&[1, 0, 10, 1, 5], 0, 10).unwrap().e.key.as_ptr(), a2.job(&[1, 0, 10, 1, 5], 0, 10).unwrap().e.key.as_ptr()));
        assert!(Coop::map(&std::env::temp_dir()).is_err(), "a directory is not a table");
        let _ = std::fs::remove_file(path);
    }
}
