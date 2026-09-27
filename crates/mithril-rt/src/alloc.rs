//! Shared arenas: cells and waiting records, carved out in fixed-size chunks
//! by a global bump pointer. Per-worker free lists live in `Wctx`.

use std::alloc::{alloc_zeroed, handle_alloc_error, Layout};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Slots handed to a worker per global bump.
pub(crate) const CHUNK: u64 = 1 << 16;

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
// SAFETY: Rec consists solely of AtomicU32/AtomicU64 fields.
unsafe impl ZeroValid for Rec {}

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
    // SAFETY: p is a fresh global-allocator block with Layout::array::<T>(n),
    // exclusively owned here, and all-zero bytes are a valid T (ZeroValid).
    unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(p, n)) }
}

/// Parse a capacity override: decimal, optional `0x` hex, or `1<<k`.
pub(crate) fn parse_cap(v: Option<&str>, default: usize) -> Result<usize, String> {
    let Some(raw) = v else { return Ok(default) };
    let s = raw.trim();
    let parsed = if let Some(k) = s.strip_prefix("1<<") {
        k.trim().parse::<u32>().ok().filter(|&k| k < 32).map(|k| 1usize << k)
    } else if let Some(h) = s.strip_prefix("0x") {
        usize::from_str_radix(h, 16).ok()
    } else {
        s.parse::<usize>().ok()
    };
    match parsed {
        Some(n) if n > 0 && n <= u32::MAX as usize => Ok(n),
        _ => Err(format!("invalid capacity {raw:?} (want 1..=2^32-1)")),
    }
}

pub(crate) fn cap_from_env(name: &str, default: usize) -> usize {
    let v = std::env::var(name).ok();
    parse_cap(v.as_deref(), default).unwrap_or_else(|e| panic!("{name}: {e}"))
}

pub(crate) struct Arena {
    /// Cell i occupies words 2i and 2i+1.
    cells: Box<[AtomicU64]>,
    pub recs: Box<[Rec]>,
    cbump: AtomicU64,
    rbump: AtomicU64,
    result: AtomicU64,
    has_result: AtomicBool,
}

impl Arena {
    pub fn new(ncells: usize, nrecs: usize) -> Arena {
        assert!(ncells <= u32::MAX as usize && nrecs <= u32::MAX as usize, "capacity exceeds u32 index space");
        Arena {
            cells: zeroed_slice(2 * ncells),
            // record 0 is the reserved ROOT sink, so keep at least one slot
            recs: zeroed_slice(nrecs.max(1)),
            cbump: AtomicU64::new(0),
            rbump: AtomicU64::new(1),
            result: AtomicU64::new(0),
            has_result: AtomicBool::new(false),
        }
    }

    /// Forget every allocation (start of a run). Contents are not cleared:
    /// every slot is written on allocation before it can be read.
    pub fn reset(&mut self) {
        *self.cbump.get_mut() = 0;
        *self.rbump.get_mut() = 1;
        *self.result.get_mut() = 0;
        *self.has_result.get_mut() = false;
    }

    fn claim(bump: &AtomicU64, cap: usize, what: &str, env: &str) -> (u32, u32) {
        let base = bump.fetch_add(CHUNK, Ordering::Relaxed);
        if base >= cap as u64 {
            panic!("arena exhausted: {what} arena full at {cap} slots (raise {env})");
        }
        (base as u32, (base + CHUNK).min(cap as u64) as u32)
    }

    pub fn claim_cells(&self) -> (u32, u32) {
        Self::claim(&self.cbump, self.cells.len() / 2, "cell", "MITHRIL_NODES")
    }

    pub fn claim_recs(&self) -> (u32, u32) {
        Self::claim(&self.rbump, self.recs.len(), "record", "MITHRIL_RECS")
    }

    #[inline]
    pub fn cell(&self, i: u32) -> [u64; 2] {
        let k = 2 * i as usize;
        [self.cells[k].load(Ordering::Relaxed), self.cells[k + 1].load(Ordering::Relaxed)]
    }

    #[inline]
    pub fn set(&self, i: u32, slot: usize, v: u64) {
        assert!(slot < 2, "cell slot {slot} out of range");
        self.cells[2 * i as usize + slot].store(v, Ordering::Relaxed);
    }

    pub fn deliver_root(&self, v: u64) {
        self.result.store(v, Ordering::Relaxed);
        if self.has_result.swap(true, Ordering::AcqRel) {
            panic!("ROOT received more than one delivery");
        }
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
        assert!(parse_cap(Some("4294967296"), 7).is_err());
    }

    #[test]
    fn claims_are_chunked_and_capped() {
        let a = Arena::new(CHUNK as usize + 10, 3);
        assert_eq!(a.claim_cells(), (0, CHUNK as u32));
        assert_eq!(a.claim_cells(), (CHUNK as u32, CHUNK as u32 + 10));
        assert_eq!(a.claim_recs(), (1, 3));
        let r = std::panic::catch_unwind(|| a.claim_cells());
        assert!(r.is_err());
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
