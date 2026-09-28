//! Per-worker context: chunked bump + free-list allocation of cells and
//! records, record delivery, and thread-local spawn buffers that the
//! coordinator merges into the global buckets after every wave.

use crate::alloc::Arena;
use crate::{DiveResult, Program, Redex};

/// Bound on records fired inline from `deliver` (each level may nest a
/// fuel-bounded dive under it).
const MAX_INLINE: u32 = 64;
use std::sync::atomic::Ordering;

/// Static payload of a waiting record, readable while its rule fires.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RecInfo {
    pub rule: u16,
    pub d: u32,
    pub s: u32,
    pub parent: u64,
}

/// A worker's view of the engine during a wave.
pub struct Wctx<'e> {
    ar: &'e Arena,
    prog: &'e dyn Program,
    fuel: i64,
    /// Intrusive free list threaded through cell word 0 (NIL = u32::MAX).
    cfree_head: u32,
    /// Current bump chunk `[lo, hi)` (u64: `hi` may be 2^32).
    cchunk: (u64, u64),
    rfree: Vec<u32>,
    rchunk: (u64, u64),
    /// Spawned redexes / activated records per rule, since the last merge.
    out: Vec<Vec<Redex>>,
    out_recs: Vec<Vec<u32>>,
    /// Rules with non-empty output buffers (each listed once).
    dirty: Vec<u16>,
    marked: Vec<bool>,
    /// Cell slots taken from the bump region (the footprint contribution).
    pub(crate) issued: usize,
    pub(crate) live: i64,
    pub(crate) live_peak: i64,
    pub(crate) rewrites: u64,
    /// Nesting of records fired inline from `deliver` (see `MAX_INLINE`).
    inline_depth: u32,
    /// `MITHRIL_CHECK_FREE` (debug builds): track freed cells and panic on
    /// a double free.
    check_free: bool,
    freed: Vec<u64>,
}

impl<'e> Wctx<'e> {
    pub(crate) fn new(ar: &'e Arena, prog: &'e dyn Program, fuel: i64, n_rules: usize) -> Wctx<'e> {
        Wctx {
            ar,
            prog,
            fuel,
            cfree_head: u32::MAX,
            cchunk: (0, 0),
            rfree: Vec::new(),
            rchunk: (0, 0),
            out: (0..n_rules).map(|_| Vec::new()).collect(),
            out_recs: (0..n_rules).map(|_| Vec::new()).collect(),
            dirty: Vec::new(),
            marked: vec![false; n_rules],
            issued: 0,
            live: 0,
            live_peak: 0,
            rewrites: 0,
            inline_depth: 0,
            check_free: std::env::var_os("MITHRIL_CHECK_FREE").is_some(),
            freed: Vec::new(),
        }
    }

    // ---- cells ----

    /// Allocate a cell holding `[a, b]`, reusing this worker's freed slots first.
    #[inline(always)]
    pub fn alloc(&mut self, a: u64, b: u64) -> u32 {
        if cfg!(debug_assertions) && self.check_free {
            let i = self.alloc_inner(a, b);
            let (w, bt) = ((i / 64) as usize, i % 64);
            if self.freed.len() > w {
                self.freed[w] &= !(1 << bt);
            }
            return i;
        }
        self.alloc_inner(a, b)
    }

    #[inline(always)]
    fn alloc_inner(&mut self, a: u64, b: u64) -> u32 {
        let i = if self.cfree_head != u32::MAX {
            let i = self.cfree_head;
            self.cfree_head = self.ar.cell(i)[0] as u32;
            i
        } else {
            if self.cchunk.0 == self.cchunk.1 {
                self.cchunk = self.ar.claim_cells();
            }
            self.issued += 1;
            self.cchunk.0 += 1;
            // chunks lie below the capacity, which is <= 2^32
            (self.cchunk.0 - 1) as u32
        };
        self.ar.set(i, 0, a);
        self.ar.set(i, 1, b);
        self.ar.rc_set1(i);
        i
    }

    /// Allocate a cell of a statically linear type (never shared): the
    /// refcount is never read, so it is never written (no rc cache line).
    #[inline(always)]
    pub fn alloc_lin(&mut self, a: u64, b: u64) -> u32 {
        if cfg!(debug_assertions) && self.check_free {
            let i = self.alloc_lin_inner(a, b);
            let (w, bt) = ((i / 64) as usize, i % 64);
            if self.freed.len() > w {
                self.freed[w] &= !(1 << bt);
            }
            return i;
        }
        self.alloc_lin_inner(a, b)
    }

    #[inline(always)]
    fn alloc_lin_inner(&mut self, a: u64, b: u64) -> u32 {
        let i = if self.cfree_head != u32::MAX {
            let i = self.cfree_head;
            self.cfree_head = self.ar.cell(i)[0] as u32;
            i
        } else {
            if self.cchunk.0 == self.cchunk.1 {
                self.cchunk = self.ar.claim_cells();
            }
            self.issued += 1;
            self.cchunk.0 += 1;
            (self.cchunk.0 - 1) as u32
        };
        self.ar.set(i, 0, a);
        self.ar.set(i, 1, b);
        i
    }

    /// O(1) share: bump cell i's refcount.
    #[inline(always)]
    pub fn rc_inc(&self, i: u32) {
        self.ar.rc_inc(i)
    }

    /// Drop one reference; true = last one (caller tears the cell down).
    #[inline(always)]
    pub fn rc_dec(&self, i: u32) -> bool {
        self.ar.rc_dec(i)
    }

    /// Reset a reused cell's refcount to one (reuse of a unique cell).
    #[inline(always)]
    pub fn rc_set1(&self, i: u32) {
        self.ar.rc_set1(i)
    }

    /// True when the caller's reference is the only one.
    #[inline(always)]
    pub fn rc_unique(&self, i: u32) -> bool {
        self.ar.rc_get(i) == 1
    }

    /// Return a cell to this worker's free list (the caller owns it linearly).
    #[inline(always)]
    pub fn free(&mut self, i: u32) {
        if cfg!(debug_assertions) && self.check_free {
            let (w, b) = ((i / 64) as usize, i % 64);
            if self.freed.len() <= w {
                self.freed.resize(w + 1, 0);
            }
            assert!(self.freed[w] & (1 << b) == 0, "double free of cell {i}");
            self.freed[w] |= 1 << b;
        }
        self.ar.set(i, 0, self.cfree_head as u64);
        self.cfree_head = i;
    }

    #[inline(always)]
    pub fn cell(&self, i: u32) -> [u64; 2] {
        self.ar.cell(i)
    }

    #[inline]
    pub fn set(&self, i: u32, slot: usize, v: u64) {
        self.ar.set(i, slot, v)
    }

    // ---- records ----

    /// Allocate a waiting record that fires `rule` after `pend` deliveries.
    pub fn alloc_rec(&mut self, rule: u16, pend: u32, d: u32, s: u32, parent: u64) -> u32 {
        assert!((rule as usize) < self.out.len(), "alloc_rec: rule {rule} out of range");
        let i = match self.rfree.pop() {
            Some(i) => i,
            None => {
                if self.rchunk.0 == self.rchunk.1 {
                    self.rchunk = self.ar.claim_recs();
                }
                self.rchunk.0 += 1;
                (self.rchunk.0 - 1) as u32
            }
        };
        let r = &self.ar.recs[i as usize];
        r.rule.store(rule as u32, Ordering::Relaxed);
        r.d.store(d, Ordering::Relaxed);
        r.s.store(s, Ordering::Relaxed);
        r.parent.store(parent, Ordering::Relaxed);
        r.pend.store(pend, Ordering::Release);
        i
    }

    /// Static payload of record `i` (valid while it is live, e.g. inside the
    /// `fire` call it activated, where `e.aux == i`).
    pub fn rec(&self, i: u32) -> RecInfo {
        let r = &self.ar.recs[i as usize];
        RecInfo {
            rule: r.rule.load(Ordering::Relaxed) as u16,
            d: r.d.load(Ordering::Relaxed),
            s: r.s.load(Ordering::Relaxed),
            parent: r.parent.load(Ordering::Relaxed),
        }
    }

    /// Re-point a live record's parent (used when a suspended dive captures
    /// its continuation as records whose destination is only known higher
    /// up the call chain). Must happen before any delivery can fire `i`.
    pub fn set_parent(&self, i: u32, parent: u64) {
        self.ar.recs[i as usize].parent.store(parent, Ordering::Relaxed);
    }

    /// Fill slot `parent & 7` of record `parent >> 3`; the delivery that
    /// brings `pend` to zero queues the record in its rule's bucket.
    /// `parent == ROOT` finishes the run with `val`.
    pub fn deliver(&mut self, parent: u64, val: u64) {
        let ri = (parent >> 3) as u32;
        if ri == 0 {
            self.ar.deliver_root(val);
            return;
        }
        let r = &self.ar.recs[ri as usize];
        r.args[(parent & 7) as usize].store(val, Ordering::Release);
        if r.pend.fetch_sub(1, Ordering::AcqRel) == 1 {
            let rule = r.rule.load(Ordering::Relaxed) as u16;
            // The last child fires the record right here: a chain of
            // nested joins then completes in one wave instead of one hop
            // per wave. Depth-bounded so a long chain cannot overrun the
            // stack; beyond it the record waits for the next wave.
            if self.inline_depth < MAX_INLINE {
                self.inline_depth += 1;
                self.fire_rec(rule, ri);
                self.inline_depth -= 1;
            } else {
                self.mark(rule);
                self.out_recs[rule as usize].push(ri);
            }
        }
    }

    /// Queue record `ri` (allocated with pend 0: a task with no inputs) to
    /// fire in a later wave.
    #[inline]
    pub fn ready_rec(&mut self, ri: u32) {
        let rule = self.ar.recs[ri as usize].rule.load(Ordering::Relaxed) as u16;
        self.mark(rule);
        self.out_recs[rule as usize].push(ri);
    }

    // ---- redexes ----

    /// Queue `e` for `rule` in a later wave.
    #[inline]
    pub fn spawn(&mut self, rule: u16, e: Redex) {
        assert!((rule as usize) < self.out.len(), "spawn: rule {rule} out of range");
        self.mark(rule);
        self.out[rule as usize].push(e);
    }

    /// Per-dive fuel budget configured on the engine.
    pub fn fuel(&self) -> i64 {
        self.fuel
    }

    /// Set the per-dive budget for the next wave (see `Engine::run`).
    pub(crate) fn set_fuel(&mut self, f: i64) {
        self.fuel = f;
    }

    /// Run `prog.dive(f, args)` with a fresh budget of `self.fuel()`.
    pub fn dive(&mut self, f: u16, args: &[u64]) -> DiveResult {
        let mut fuel = self.fuel;
        let prog = self.prog;
        prog.dive(f, args, &mut fuel, self)
    }

    // ---- engine side ----

    #[inline]
    fn mark(&mut self, rule: u16) {
        if !self.marked[rule as usize] {
            self.marked[rule as usize] = true;
            self.dirty.push(rule);
        }
    }

    #[inline]
    pub(crate) fn fire_redex(&mut self, rule: u16, e: Redex) {
        self.rewrites += 1;
        let prog = self.prog;
        prog.fire(rule, e, self);
    }

    /// Fire an activated record, then recycle it.
    #[inline]
    pub(crate) fn fire_rec(&mut self, rule: u16, ri: u32) {
        let r = &self.ar.recs[ri as usize];
        let e = Redex { a: r.args[0].load(Ordering::Acquire), b: r.args[1].load(Ordering::Acquire), aux: ri as u64 };
        self.fire_redex(rule, e);
        self.rfree.push(ri);
    }

    /// Move this worker's output buffers into the global buckets.
    pub(crate) fn merge_into(&mut self, redexes: &mut [Vec<Redex>], recs: &mut [Vec<u32>]) {
        for rule in self.dirty.drain(..) {
            let k = rule as usize;
            self.marked[k] = false;
            redexes[k].append(&mut self.out[k]);
            recs[k].append(&mut self.out_recs[k]);
        }
    }
}
