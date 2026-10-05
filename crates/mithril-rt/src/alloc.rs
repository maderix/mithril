//! Shared arenas: cells and waiting records, carved out in fixed-size chunks
//! by a global bump pointer. Per-worker free lists live in `Wctx`.

use std::alloc::{alloc_zeroed, handle_alloc_error, Layout};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Slots handed to a worker per global bump.
pub(crate) const CHUNK: u64 = 1 << 16;

/// Largest arena capacity: slot indices are `u32` in the public API
/// (`Wctx::alloc`, `Engine::cell`, record indices), so 2^32 slots
/// (indices 0..=u32::MAX) is the index-width limit, not a tuning clamp.
/// Arenas are reserved up front but only committed as chunks are touched,
/// so a large capacity grows physical memory chunk by chunk.
pub(crate) const MAX_SLOTS: usize = 1 << 32;

/// Waiting record: `pend` deliveries outstanding, two argument slots, and the
/// static payload (`rule`, `d`, `s`, `parent`) fixed at allocation.
pub(crate) struct Rec {
    pub pend: AtomicU32,
    pub rule: AtomicU32,
    pub d: AtomicU32,
    pub s: AtomicU32,
    pub parent: AtomicU64,
    pub args: [AtomicU64; 2],
}

/// Types for which the all-zero bit pattern is a valid value.
///
/// # Safety
/// Implementors must be valid when every byte is zero.
unsafe trait ZeroValid {}
// SAFETY: AtomicU64 has the same in-memory representation as u64.
unsafe impl ZeroValid for AtomicU64 {}
// SAFETY: all-zero bytes are a valid AtomicU32 (value 0).
unsafe impl ZeroValid for std::sync::atomic::AtomicU32 {}
// SAFETY: Rec consists solely of AtomicU32/AtomicU64 fields.
unsafe impl ZeroValid for Rec {}

/// Ask for transparent huge pages on an arena block (Linux; the kernel's
/// default THP mode is `madvise`, so without this every arena is 4 KiB
/// pages and pointer-chasing workloads pay a TLB miss per node). Best
/// effort: failure leaves ordinary pages.
fn advise_huge(p: *mut u8, len: usize) {
    #[cfg(target_os = "linux")]
    {
        const MADV_HUGEPAGE: i32 = 14;
        const HUGE: usize = 2 << 20;
        extern "C" {
            fn madvise(addr: *mut u8, len: usize, advice: i32) -> i32;
        }
        let start = (p as usize).next_multiple_of(HUGE);
        let end = (p as usize + len) & !(HUGE - 1);
        if end > start {
            // SAFETY: [start, end) lies inside the block just allocated;
            // MADV_HUGEPAGE changes no contents, only the backing page size.
            unsafe {
                madvise(start as *mut u8, end - start, MADV_HUGEPAGE);
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (p, len);
}

/// Allocate `n` zeroed values without touching the pages (calloc keeps large
/// arenas lazily committed, so a 1 GiB default capacity costs nothing upfront).
fn zeroed_slice<T: ZeroValid>(n: usize) -> Box<[T]> {
    if n == 0 {
        return Vec::new().into_boxed_slice();
    }
    let layout = Layout::array::<T>(n).expect("arena size overflows address space");
    // SAFETY: layout has non-zero size (n > 0 and T is not zero-sized).
    let p = unsafe { alloc_zeroed(layout) } as *mut T;
    if p.is_null() {
        handle_alloc_error(layout);
    }
    advise_huge(p as *mut u8, layout.size());
    // SAFETY: p is a fresh global-allocator block with Layout::array::<T>(n),
    // exclusively owned here, and all-zero bytes are a valid T (ZeroValid).
    unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(p, n)) }
}

/// Parse a capacity override: decimal, optional `0x` hex, or `1<<k`.
pub(crate) fn parse_cap(v: Option<&str>, default: usize) -> Result<usize, String> {
    let Some(raw) = v else { return Ok(default) };
    let s = raw.trim();
    let parsed = if let Some(k) = s.strip_prefix("1<<") {
        k.trim().parse::<u32>().ok().filter(|&k| k <= 32).map(|k| 1usize << k)
    } else if let Some(h) = s.strip_prefix("0x") {
        usize::from_str_radix(h, 16).ok()
    } else {
        s.parse::<usize>().ok()
    };
    match parsed {
        Some(n) if n > 0 && n <= MAX_SLOTS => Ok(n),
        _ => Err(format!("invalid capacity {raw:?} (want 1..=2^32, the u32 index range)")),
    }
}

pub(crate) fn cap_from_env(name: &str, default: usize) -> usize {
    let v = std::env::var(name).ok();
    parse_cap(v.as_deref(), default).unwrap_or_else(|e| panic!("{name}: {e}"))
}

pub(crate) struct Arena {
    /// Dup label supply shared by every worker (24-bit, wraps).
    pub(crate) labels: std::sync::atomic::AtomicU32,
    /// Cell i occupies words 2i and 2i+1.
    cells: Box<[AtomicU64]>,
    /// Reference count of cell i (valid while allocated; alloc sets 1).
    /// per-cell reference counts of shared (non-linear) constructors
    rc: Box<[std::sync::atomic::AtomicU32]>,
    pub recs: Box<[Rec]>,
    cbump: AtomicU64,
    rbump: AtomicU64,
    result: AtomicU64,
    has_result: AtomicBool,
    /// slot 1 of record 0: a co-execution chunk's result (see `coop`)
    coop: AtomicU64,
    has_coop: AtomicBool,
    /// Striped locks over cells: copying a closure rewrites its cell, and the
    /// closure may sit in a structure several workers share.
    locks: Box<[AtomicBool]>,
}

/// Stripes of `Arena::locks`.
const LOCK_STRIPES: usize = 4096;

impl Arena {
    pub fn new(ncells: usize, nrecs: usize) -> Arena {
        assert!(ncells <= MAX_SLOTS && nrecs <= MAX_SLOTS, "capacity exceeds the u32 index range (2^32 slots)");
        Arena {
            labels: std::sync::atomic::AtomicU32::new(1),
            locks: (0..LOCK_STRIPES).map(|_| AtomicBool::new(false)).collect(),
            cells: zeroed_slice(2 * ncells),
            rc: zeroed_slice(ncells),
            // record 0 is the reserved ROOT sink, so keep at least one slot
            recs: zeroed_slice(nrecs.max(1)),
            cbump: AtomicU64::new(0),
            rbump: AtomicU64::new(1),
            result: AtomicU64::new(0),
            has_result: AtomicBool::new(false),
            coop: AtomicU64::new(0),
            has_coop: AtomicBool::new(false),
        }
    }

    /// Forget every allocation (start of a run). Contents are not cleared:
    /// every slot is written on allocation before it can be read.
    pub fn reset(&mut self) {
        *self.cbump.get_mut() = 0;
        *self.rbump.get_mut() = 1;
        *self.result.get_mut() = 0;
        *self.has_result.get_mut() = false;
        *self.has_coop.get_mut() = false;
    }

    /// Claim the next chunk `[lo, hi)`; `hi` may be 2^32, hence u64. A
    /// small arena hands out smaller chunks (1/256 of it, at least 64), so
    /// several workers can share it.
    fn claim(bump: &AtomicU64, cap: usize, what: &str, env: &str) -> (u64, u64) {
        let chunk = CHUNK.min(((cap as u64) / 256).max(64));
        let base = bump.fetch_add(chunk, Ordering::Relaxed);
        if base >= cap as u64 {
            panic!(
                "arena exhausted: {what} arena full at {cap} slots \
                 (raise {env}, max {MAX_SLOTS} = 2^32, e.g. {env}=1<<28)"
            );
        }
        (base, (base + chunk).min(cap as u64))
    }

    pub fn claim_cells(&self) -> (u64, u64) {
        Self::claim(&self.cbump, self.cells.len() / 2, "cell", "MITHRIL_NODES")
    }

    pub fn claim_recs(&self) -> (u64, u64) {
        Self::claim(&self.rbump, self.recs.len(), "record", "MITHRIL_RECS")
    }

    #[inline]
    pub fn cell(&self, i: u32) -> [u64; 2] {
        let k = 2 * i as usize;
        debug_assert!(k + 1 < self.cells.len());
        // SAFETY: cell indices only come from the allocator (claim/free
        // lists), which never issues a slot at or beyond capacity.
        unsafe {
            [
                self.cells.get_unchecked(k).load(Ordering::Relaxed),
                self.cells.get_unchecked(k + 1).load(Ordering::Relaxed),
            ]
        }
    }

    #[inline(always)]
    fn rcs(&self, i: u32) -> &std::sync::atomic::AtomicU32 {
        debug_assert!((i as usize) < self.rc.len());
        // SAFETY: same allocator-issued index invariant as `cell`.
        unsafe { self.rc.get_unchecked(i as usize) }
    }

    /// 32-bit counts (as the device's): an 8-bit count saturating at 255
    /// wrapped through 0 for a moment under concurrent increments, and a
    /// reader then saw the cell as unique and freed a shared tree.
    #[inline(always)]
    pub fn rc_inc(&self, i: u32) {
        crate::sync::rc_share::<u32, _>(self.rcs(i));
    }

    /// Decrement; returns true when this was the last reference (the caller
    /// then owns the cell's teardown). AcqRel (`sync::rc_release`): other
    /// owners' reads happen before the last owner's teardown and reuse.
    #[inline(always)]
    pub fn rc_dec(&self, i: u32) -> bool {
        crate::sync::rc_release(self.rcs(i))
    }

    #[inline(always)]
    pub fn rc_set1(&self, i: u32) {
        self.rcs(i).store(1, Ordering::Relaxed);
    }

    /// True when the caller holds the only reference (`sync::rc_unique`).
    #[inline(always)]
    pub fn rc_unique(&self, i: u32) -> bool {
        crate::sync::rc_unique(self.rcs(i))
    }

    /// Hold cell `i`'s stripe until `unlock`.
    pub fn lock(&self, i: u32) {
        let l = &self.locks[i as usize % LOCK_STRIPES];
        while l.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            std::hint::spin_loop();
        }
    }

    pub fn unlock(&self, i: u32) {
        self.locks[i as usize % LOCK_STRIPES].store(false, Ordering::Release);
    }

    /// Store `v` in slot 0 of cell `i` if it holds `empty`, or return what
    /// it holds. Release on the store, acquire on the read: the cells behind
    /// the port are visible to the end that takes it.
    #[inline(always)]
    pub fn install(&self, i: u32, empty: u64, v: u64) -> Option<u64> {
        let k = 2 * i as usize;
        debug_assert!(k < self.cells.len());
        // SAFETY: allocator-issued index.
        let slot = unsafe { self.cells.get_unchecked(k) };
        slot.compare_exchange(empty, v, Ordering::AcqRel, Ordering::Acquire).err()
    }

    #[inline(always)]
    pub fn set(&self, i: u32, slot: usize, v: u64) {
        let k = 2 * i as usize + slot;
        debug_assert!(slot < 2 && k < self.cells.len());
        // SAFETY: allocator-issued index; slot is 0 or 1 at every call site.
        unsafe { self.cells.get_unchecked(k).store(v, Ordering::Relaxed) }
    }

    pub fn deliver_root(&self, v: u64) {
        self.result.store(v, Ordering::Relaxed);
        if self.has_result.swap(true, Ordering::AcqRel) {
            panic!("ROOT received more than one delivery");
        }
    }

    pub fn deliver_coop(&self, v: u64) {
        self.coop.store(v, Ordering::Relaxed);
        if self.has_coop.swap(true, Ordering::AcqRel) {
            panic!("a co-execution chunk delivered twice");
        }
    }

    /// The chunk result delivered since the last take, if any.
    pub fn take_coop(&self) -> Option<u64> {
        self.has_coop.swap(false, Ordering::AcqRel).then(|| self.coop.load(Ordering::Relaxed))
    }

    pub fn result(&self) -> Option<u64> {
        self.has_result.load(Ordering::Acquire).then(|| self.result.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cap_forms() {
        assert_eq!(parse_cap(None, 7), Ok(7));
        assert_eq!(parse_cap(Some("1234"), 7), Ok(1234));
        assert_eq!(parse_cap(Some(" 0x100 "), 7), Ok(256));
        assert_eq!(parse_cap(Some("1<<20"), 7), Ok(1 << 20));
        assert!(parse_cap(Some("0"), 7).is_err());
        assert!(parse_cap(Some("lots"), 7).is_err());
        assert!(parse_cap(Some("1<<40"), 7).is_err());
        assert!(parse_cap(Some("1<<33"), 7).is_err());
        assert_eq!(parse_cap(Some("1<<32"), 7), Ok(1 << 32));
        assert_eq!(parse_cap(Some("4294967296"), 7), Ok(1 << 32));
        assert!(parse_cap(Some("4294967297"), 7).is_err());
    }

    #[test]
    fn claims_are_chunked_and_capped() {
        // a large arena hands out full chunks, the last one capped
        let big = 256 * CHUNK as usize + 10;
        let a = Arena::new(big, 3);
        assert_eq!(a.claim_cells(), (0, CHUNK));
        assert_eq!(a.claim_cells(), (CHUNK, 2 * CHUNK));
        // a small arena hands out 1/256 of itself, at least 64 slots
        let s = Arena::new(1000, 3);
        assert_eq!(s.claim_cells(), (0, 64));
        assert_eq!(s.claim_cells(), (64, 128));
        let m = Arena::new(1 << 16, 3);
        assert_eq!(m.claim_cells(), (0, 256));
        // exhaustion (and the cap on the last chunk)
        let a = Arena::new(64 + 10, 3);
        assert_eq!(a.claim_cells(), (0, 64));
        assert_eq!(a.claim_cells(), (64, 74));
        assert_eq!(a.claim_recs(), (1, 3));
        let r = std::panic::catch_unwind(|| a.claim_cells());
        let msg = *r.unwrap_err().downcast::<String>().unwrap();
        assert!(msg.contains("arena exhausted") && msg.contains("MITHRIL_NODES"), "{msg}");
        let r = std::panic::catch_unwind(|| a.claim_recs());
        let msg = *r.unwrap_err().downcast::<String>().unwrap();
        assert!(msg.contains("record arena") && msg.contains("MITHRIL_RECS"), "{msg}");
    }

    #[test]
    fn zeroed_arena_and_root_result() {
        let mut a = Arena::new(4, 4);
        assert_eq!(a.cell(3), [0, 0]);
        a.set(3, 1, 9);
        assert_eq!(a.cell(3), [0, 9]);
        assert_eq!(a.result(), None);
        a.deliver_root(5);
        assert_eq!(a.result(), Some(5));
        a.reset();
        assert_eq!(a.result(), None);
        assert_eq!(a.claim_recs(), (1, 4));
    }
}

#[cfg(test)]
mod top_chunk {
    use super::*;
    use std::sync::atomic::AtomicU64;

    #[test]
    fn last_chunk_of_full_u32_range_ends_at_2_pow_32() {
        // No 64 GiB arena needed: exercise the chunk arithmetic directly.
        let bump = AtomicU64::new(MAX_SLOTS as u64 - CHUNK);
        let (lo, hi) = Arena::claim(&bump, MAX_SLOTS, "cell", "MITHRIL_NODES");
        assert_eq!((lo, hi), (MAX_SLOTS as u64 - CHUNK, MAX_SLOTS as u64));
        assert_eq!((hi - 1) as u32, u32::MAX);
        assert!(std::panic::catch_unwind(|| Arena::claim(&bump, MAX_SLOTS, "cell", "X")).is_err());
    }
}
