//! The runtime's lockless protocols: a join record's arrival, a wave's
//! claim cursor, and refcount share/release. They are generic over the
//! atomic types so the loom models (`tests/loom_test.rs`, feature `loom`)
//! check the very code the runtime runs, with every interleaving and
//! memory ordering the C11 model allows.

use std::ops::Range;
use std::sync::atomic::Ordering;

/// The atomic operations the protocols use (std's and loom's types).
pub trait AtomicWord<T> {
    fn load(&self, o: Ordering) -> T;
    fn store(&self, v: T, o: Ordering);
    fn fetch_add(&self, v: T, o: Ordering) -> T;
    fn fetch_sub(&self, v: T, o: Ordering) -> T;
}

macro_rules! atomic_word {
    ($($a:ty => $t:ty),*) => {$(
        impl AtomicWord<$t> for $a {
            #[inline(always)]
            fn load(&self, o: Ordering) -> $t { <$a>::load(self, o) }
            #[inline(always)]
            fn store(&self, v: $t, o: Ordering) { <$a>::store(self, v, o) }
            #[inline(always)]
            fn fetch_add(&self, v: $t, o: Ordering) -> $t { <$a>::fetch_add(self, v, o) }
            #[inline(always)]
            fn fetch_sub(&self, v: $t, o: Ordering) -> $t { <$a>::fetch_sub(self, v, o) }
        }
    )*};
}

atomic_word!(std::sync::atomic::AtomicU32 => u32, std::sync::atomic::AtomicU64 => u64, std::sync::atomic::AtomicUsize => usize);
#[cfg(feature = "loom")]
atomic_word!(loom::sync::atomic::AtomicU32 => u32, loom::sync::atomic::AtomicU64 => u64, loom::sync::atomic::AtomicUsize => usize);

/// A value arrives at a join record: publish it, then count down. True for
/// exactly one caller, the last, which then reads every argument.
#[inline(always)]
pub fn join_arrive<P: AtomicWord<u32>, A: AtomicWord<u64>>(pend: &P, arg: &A, val: u64) -> bool {
    arg.store(val, Ordering::Release);
    pend.fetch_sub(1, Ordering::AcqRel) == 1
}

/// The arguments of a record whose last arrival this thread saw.
#[inline(always)]
pub fn join_args<A: AtomicWord<u64>>(args: &[A; 2]) -> (u64, u64) {
    (args[0].load(Ordering::Acquire), args[1].load(Ordering::Acquire))
}

/// A wave's shared cursor: the next block of `[0, n)`, or `None` when the
/// wave is drained.
#[inline(always)]
pub fn claim_block<C: AtomicWord<usize>>(next: &C, block: usize, n: usize) -> Option<Range<usize>> {
    let i = next.fetch_add(block, Ordering::Relaxed);
    if i >= n {
        None
    } else {
        Some(i..(i + block).min(n))
    }
}

/// One more owner of a refcounted cell or array block.
#[inline(always)]
pub fn rc_share<T: From<u8>, C: AtomicWord<T>>(rc: &C) {
    rc.fetch_add(T::from(1), Ordering::Relaxed);
}

/// The caller's reference is the only one, so it may free or reuse the
/// cell in place. Acquire: every other owner's reads of the contents
/// happen before (they end in `rc_release`). A Relaxed check let the reuse
/// race another owner's read (loom: `unique_owner_reuses_after_other_reads`).
#[inline(always)]
pub fn rc_unique<T: From<u8> + PartialEq, C: AtomicWord<T>>(rc: &C) -> bool {
    rc.load(Ordering::Acquire) == T::from(1)
}

/// An owner of a refcounted cell or array block lets go; true when it was
/// the last. AcqRel: the other owners' reads of the contents happen before
/// this decrement (Release), and the last owner's teardown and reuse happen
/// after it (Acquire). A Release-only decrement let the last owner's reuse
/// race another owner's read (loom: `cell_release_orders_reads_before_reuse`).
#[inline(always)]
pub fn rc_release<T: From<u8> + PartialEq, C: AtomicWord<T>>(rc: &C) -> bool {
    rc.fetch_sub(T::from(1), Ordering::AcqRel) == T::from(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize};

    #[test]
    fn join_fires_only_on_the_last_arrival() {
        let pend = AtomicU32::new(2);
        let args = [AtomicU64::new(0), AtomicU64::new(0)];
        assert!(!join_arrive(&pend, &args[1], 7));
        assert!(join_arrive(&pend, &args[0], 5));
        assert_eq!(join_args(&args), (5, 7));
    }

    #[test]
    fn claim_blocks_cover_the_range_once() {
        let next = AtomicUsize::new(0);
        let mut seen = Vec::new();
        while let Some(r) = claim_block(&next, 2, 5) {
            seen.extend(r);
        }
        assert_eq!(seen, vec![0, 1, 2, 3, 4]);
        assert_eq!(claim_block(&next, 2, 5), None);
    }

    #[test]
    fn refcount_release_reports_the_last_owner() {
        let rc = AtomicU32::new(1);
        rc_share::<u32, _>(&rc);
        assert!(!rc_unique(&rc));
        assert!(!rc_release(&rc));
        assert!(rc_unique(&rc));
        assert!(rc_release(&rc));
        let arc = AtomicU64::new(2);
        assert!(!rc_release(&arc));
        assert!(rc_release(&arc));
    }
}
