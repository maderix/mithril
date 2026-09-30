//! mithril-rt: the generic batched wave engine (CPU).
//!
//! A compiled Mithril program implements [`Program`]: a set of numbered
//! rewrite rules (`fire`) plus fuel-bounded sequential forms (`dive`). The
//! [`Engine`] owns the cell and record arenas and drains per-rule redex
//! buckets wave by wave, picking the bucket with the largest estimated work
//! (`entries x rule_cost`) and running it across a persistent worker pool
//! when that work is large enough (>= 2^14), otherwise on the coordinator.
//!
//! # Protocol (what generated code may rely on)
//!
//! * **Boot.** `Engine::run(prog, boot)` fires `boot` as rule **0**. The run
//!   ends when every bucket is empty; it returns the value delivered to
//!   [`ROOT`] (panics if nothing was delivered there).
//! * **Spawned redexes.** `ctx.spawn(rule, e)` queues `e` in bucket `rule`;
//!   it fires in a later wave as `prog.fire(rule, e, ctx)`.
//! * **Records** (waiting joins). `ctx.alloc_rec(rule, pend, d, s, parent)`
//!   returns index `r`; its two argument slots are addressed as
//!   `r << 3 | slot` (slot 0 or 1). `ctx.deliver(r << 3 | slot, v)` stores `v`
//!   and decrements `pend`; when it reaches zero the record is *queued* (never
//!   fired inline, so arbitrarily deep chains cannot overflow the stack) in
//!   bucket `rule` and later fires as
//!   `prog.fire(rule, Redex { a: arg0, b: arg1, aux: r }, ctx)`. During that
//!   call `ctx.rec(r)` exposes `d`, `s` and `parent`; the engine frees the
//!   record after `fire` returns. Records are recycled through per-worker
//!   free lists. `pend` must be >= 1.
//! * **Root.** Record 0 is reserved as the result sink: [`ROOT`] (`0`) is the
//!   parent address the boot redex should deliver its final value to.
//! * **Dives.** `ctx.dive(f, args)` calls `prog.dive` with a fresh budget of
//!   the engine's fuel (also readable as `ctx.fuel()`). `DiveResult::Suspended`
//!   means the dive already spawned/recorded its residue, which will deliver
//!   to wherever the dive's result was due.
//! * **Cells** are `[u64; 2]` in a shared arena, addressed by `u32`. A cell
//!   written in one wave is visible to every worker in later waves, and to the
//!   receiver of any `deliver` that follows the write.
//! * **Capacity.** Arenas are sized by `MITHRIL_NODES` (cells, default 2^26)
//!   and `MITHRIL_RECS` (records, default 2^27), at most 2^32 each because
//!   slot indices are `u32`. Arenas are reserved virtually and committed as
//!   chunks are touched, so they grow chunk by chunk up to the capacity;
//!   exhaustion panics with "arena exhausted" naming the env var to raise.

mod alloc;
mod engine;
mod worker;
pub mod prelude;
pub mod sync;
pub mod template;

pub use engine::Engine;
pub use mithril_core;
pub use mithril_core::port::{Port, Tag};
pub use worker::{RecInfo, Wctx};

/// Parent address of the reserved root record: deliver here to finish.
pub const ROOT: u64 = 0;

/// One queued rewrite: two operands plus an auxiliary word, all raw u64s
/// whose meaning is fixed by the rule that consumes them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Redex {
    pub a: u64,
    pub b: u64,
    pub aux: u64,
}

/// Outcome of a sequential dive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DiveResult {
    /// Finished within fuel; carries the result (a Port raw in generated code).
    Done(u64),
    /// Fuel ran out; the dive has already queued its residue. Carries the
    /// root record still awaiting a parent when the dive was started with
    /// no destination (`NO_REC` when the residue already has one).
    Suspended(u32),
}

/// `DiveResult::Suspended` payload: the residue already has its destination.
pub const NO_REC: u32 = u32::MAX;

/// A compiled program: numbered rules plus fuel-bounded sequential forms.
pub trait Program: Sync {
    fn n_rules(&self) -> usize;
    /// The bucket that fires generic net redexes (`Wctx::reduce_net`
    /// spills its unfinished worklist there). `u16::MAX`: the program has
    /// no net region.
    fn net_rule(&self) -> u16 {
        u16::MAX
    }
    /// Work estimate per bucket entry (used for scheduling; 0 is treated as 1).
    fn rule_cost(&self, rule: u16) -> u32;
    fn fire(&self, rule: u16, e: Redex, ctx: &mut Wctx);
    fn dive(&self, f: u16, args: &[u64], fuel: &mut i64, ctx: &mut Wctx) -> DiveResult;
}

/// Per-run statistics, reset at the start of every `Engine::run`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Stats {
    /// Sum of per-worker peak live cells (allocs minus frees).
    pub live_peak: i64,
    /// Cell-arena footprint: distinct cell slots handed out during the run
    /// (free-list reuse keeps this near the live high-water mark).
    pub peak_cells: usize,
    /// Waves drained across the worker pool rather than on the coordinator.
    pub parallel_waves: usize,
    /// Total `fire` calls (spawned redexes plus activated records).
    pub rewrites: u64,
}
