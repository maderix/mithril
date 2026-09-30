//! Loom models of the runtime's lockless protocols: every interleaving and
//! every memory ordering the C11 model allows, on the functions the runtime
//! calls (`mithril_rt::sync`). Run:
//! `cargo test -p mithril-rt --features loom --release --test loom_test`
#![cfg(feature = "loom")]

use loom::cell::UnsafeCell;
use loom::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize};
use loom::sync::Arc;
use loom::thread;
use mithril_rt::sync::{claim_block, join_args, join_arrive, rc_release, rc_unique};
use std::sync::atomic::Ordering::Relaxed;

/// Two values arrive at a record waiting for two: exactly one arrival fires
/// it, and the firer sees both values.
#[test]
fn join_fires_once_and_sees_both_arguments() {
    loom::model(|| {
        let pend = Arc::new(AtomicU32::new(2));
        let args = Arc::new([AtomicU64::new(0), AtomicU64::new(0)]);
        let fired = Arc::new(AtomicUsize::new(0));
        let hs: Vec<_> = (0..2)
            .map(|slot| {
                let (pend, args, fired) = (pend.clone(), args.clone(), fired.clone());
                thread::spawn(move || {
                    if join_arrive(&*pend, &args[slot], 10 + slot as u64) {
                        assert_eq!(join_args(&args), (10, 11));
                        fired.fetch_add(1, Relaxed);
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(fired.load(Relaxed), 1);
    });
}

/// Three arrivals (a record counting settled fields: several arrivals write
/// the same slot): still exactly one firing.
#[test]
fn join_with_three_arrivals_fires_once() {
    loom::model(|| {
        let pend = Arc::new(AtomicU32::new(3));
        let args = Arc::new([AtomicU64::new(0), AtomicU64::new(0)]);
        let fired = Arc::new(AtomicUsize::new(0));
        let hs: Vec<_> = (0..3)
            .map(|k| {
                let (pend, args, fired) = (pend.clone(), args.clone(), fired.clone());
                thread::spawn(move || {
                    if join_arrive(&*pend, &args[k % 2], 1) {
                        fired.fetch_add(1, Relaxed);
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(fired.load(Relaxed), 1);
    });
}

/// Two workers drain a wave of 5 entries in blocks of 2: every entry is
/// claimed exactly once.
#[test]
fn claim_cursor_hands_out_each_entry_once() {
    loom::model(|| {
        let next = Arc::new(AtomicUsize::new(0));
        let hs: Vec<_> = (0..2)
            .map(|_| {
                let next = next.clone();
                thread::spawn(move || {
                    let mut got = Vec::new();
                    while let Some(r) = claim_block(&*next, 2, 5) {
                        got.extend(r);
                    }
                    got
                })
            })
            .collect();
        let mut all: Vec<usize> = hs.into_iter().flat_map(|h| h.join().unwrap()).collect();
        all.sort_unstable();
        assert_eq!(all, vec![0, 1, 2, 3, 4]);
    });
}

/// Two owners of an array block read its contents and let go; the last one
/// frees and reuses the block (a write). The reuse must not race the other
/// owner's read.
#[test]
fn array_release_orders_reads_before_reuse() {
    loom::model(|| {
        let rc = Arc::new(AtomicU64::new(2));
        let data = Arc::new(UnsafeCell::new(7u64));
        let hs: Vec<_> = (0..2)
            .map(|_| {
                let (rc, data) = (rc.clone(), data.clone());
                thread::spawn(move || {
                    let v = data.with(|p| unsafe { *p });
                    assert_eq!(v, 7);
                    if rc_release(&*rc) {
                        data.with_mut(|p| unsafe { *p = 0 });
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
    });
}

/// The same for a cell: two owners read it; the last frees and reuses it.
#[test]
fn cell_release_orders_reads_before_reuse() {
    loom::model(|| {
        let rc = Arc::new(AtomicU32::new(2));
        let data = Arc::new(UnsafeCell::new(7u64));
        let hs: Vec<_> = (0..2)
            .map(|_| {
                let (rc, data) = (rc.clone(), data.clone());
                thread::spawn(move || {
                    let v = data.with(|p| unsafe { *p });
                    assert_eq!(v, 7);
                    if rc_release(&*rc) {
                        data.with_mut(|p| unsafe { *p = 0 });
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
    });
}

/// One owner reads the cell and lets go; the other checks it is now the
/// only owner and reuses the cell in place (a consume's unique path). The
/// reuse must not race the first owner's read.
#[test]
fn unique_owner_reuses_after_other_reads() {
    loom::model(|| {
        let rc = Arc::new(AtomicU32::new(2));
        let data = Arc::new(UnsafeCell::new(7u64));
        let (rc2, data2) = (rc.clone(), data.clone());
        let reader = thread::spawn(move || {
            let v = data2.with(|p| unsafe { *p });
            assert_eq!(v, 7);
            assert!(!rc_release(&*rc2));
        });
        if rc_unique(&*rc) {
            data.with_mut(|p| unsafe { *p = 0 });
        }
        reader.join().unwrap();
    });
}
