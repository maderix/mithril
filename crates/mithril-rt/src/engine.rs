//! The wave scheduler and persistent worker pool.
//!
//! Each wave the coordinator merges every worker's spawn buffers into the
//! global per-rule buckets, picks the bucket with the largest estimated work
//! (`entries x rule_cost`, ties to the lowest rule), and drains it either
//! on its own context (small waves) or across the pool (work >= 2^14). Pool
//! workers are spawned at most once per run and park on a condvar between
//! waves; within a parallel wave they claim blocks of entries from a shared
//! atomic cursor. A parallel wave's dives get a budget proportional to the
//! entries per worker (`wave_fuel`): suspension splits work only while the
//! frontier is thin.

use crate::alloc::{cap_from_env, Arena};
use crate::worker::Wctx;
use crate::{Program, Redex, Stats};
use std::any::Any;
use std::mem;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError, RwLock};
use std::thread;

/// Cap of the sequential-chain budget boost (x base fuel).
const MAX_BOOST: i64 = 64;
static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
/// A bucket is drained in parallel when `entries x cost` reaches this.
const PAR_WORK: u128 = 1 << 14;
/// Stack for pool workers (native recursion inside fuel-bounded dives).
const WORKER_STACK: usize = 16 << 20;
/// Spins (`spin_loop` hints, tens of microseconds) a worker waits for the
/// next wave, and the coordinator for a wave's end, before parking: a
/// futex wake of every worker per wave cost knapsack at 16 threads 0.66 s
/// against 0.44 s at one (40,000 back-to-back waves of a few microseconds).
const SPIN: u32 = 1 << 14;

pub struct Engine {
    threads: usize,
    fuel: i64,
    arena: Arena,
    stats: Stats,
}

/// `MemAvailable` from /proc/meminfo, in bytes (Linux; None elsewhere).
fn mem_available() -> Option<usize> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = s.lines().find(|l| l.starts_with("MemAvailable:"))?;
    line.split_whitespace().nth(1)?.parse::<usize>().ok().map(|kb| kb * 1024)
}

impl Engine {
    /// Engine with capacities from `MITHRIL_NODES` (default 2^26 cells) /
    /// `MITHRIL_RECS` (default 2^27 records). Both are reservations that
    /// commit memory only as chunks are used; max 2^32 each (u32 indices).
    pub fn new(threads: usize, fuel: i64) -> Engine {
        let mut cells = cap_from_env("MITHRIL_NODES", 1 << 26);
        let mut recs = cap_from_env("MITHRIL_RECS", 1 << 27);
        // never more than half of the memory available now: a runaway
        // program then ends in "arena exhausted", not in swap
        if let Some(avail) = mem_available() {
            let (cb, rb) = (cells * 20, recs * 48);
            if cb + rb > avail / 2 {
                let scale = (avail / 2) as f64 / (cb + rb) as f64;
                cells = ((cells as f64 * scale) as usize).max(1 << 16);
                recs = ((recs as f64 * scale) as usize).max(1 << 12);
            }
        }
        Engine::with_capacity(threads, fuel, cells, recs)
    }

    /// Engine with explicit arena capacities (cells, records).
    pub fn with_capacity(threads: usize, fuel: i64, cells: usize, recs: usize) -> Engine {
        Engine { threads: threads.max(1), fuel: fuel.max(1), arena: Arena::new(cells, recs), stats: Stats::default() }
    }

    /// Statistics of the most recent run.
    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Read a cell after (or between) runs, e.g. for printing the result.
    pub fn cell(&self, i: u32) -> [u64; 2] {
        self.arena.cell(i)
    }

    /// Fire `boot` as rule 0 and reduce to quiescence; returns the value
    /// delivered to `ROOT`. Panics on arena exhaustion ("arena exhausted"),
    /// if no value reached `ROOT`, or if any rule panicked.
    pub fn run(&mut self, prog: &dyn Program, boot: Redex) -> u64 {
        let n_rules = prog.n_rules();
        assert!((1..=1 << 16).contains(&n_rules), "n_rules must be in 1..=65536");
        self.arena.reset();
        let (threads, fuel) = (self.threads, self.fuel);
        let ar = &self.arena;
        let costs: Vec<u128> = (0..n_rules).map(|r| prog.rule_cost(r as u16).max(1) as u128).collect();

        let mut ctx0 = Wctx::new(ar, prog, fuel, n_rules);
        let workers: Vec<Mutex<Wctx>> = (1..threads).map(|_| Mutex::new(Wctx::new(ar, prog, fuel, n_rules))).collect();
        let pool = Pool { spins: if threads <= physical_cores() { SPIN } else { 0 }, ..Pool::default() };
        let mut buckets: Vec<Vec<Redex>> = vec![Vec::new(); n_rules];
        let mut recs: Vec<Vec<u32>> = vec![Vec::new(); n_rules];
        let mut parallel_waves = 0;
        // Budget boost while suspensions expose no new work (a sequential
        // chain): doubles each wave whose frontier did not grow, capped;
        // any growth resets it. See `wave_fuel`.
        let mut boost: i64 = 1;
        let mut prev_n: usize = 0;
        ctx0.spawn(0, boot);

        thread::scope(|sc| {
            let _quit = QuitOnDrop(&pool);
            let mut spawned = false;
            loop {
                ctx0.merge_into(&mut buckets, &mut recs);
                // range requests run first, as one wave each round
                let mut ranges = mem::take(&mut ctx0.range_reqs);
                for w in &workers {
                    ranges.append(&mut lock(w).range_reqs);
                }
                if !ranges.is_empty() {
                    let total: u64 = ranges.iter().map(|r| (r.hi - r.lo).max(0) as u64).sum();
                    let sums: Vec<std::sync::atomic::AtomicU64> = ranges.iter().map(|_| std::sync::atomic::AtomicU64::new(0)).collect();
                    if threads == 1 || total < RANGE_PAR {
                        for (q, r) in ranges.iter().enumerate() {
                            let v = prog.range_leaf(r.fid, r.lo, r.hi, &r.ports, &mut ctx0);
                            sums[q].fetch_add(v as u64, Ordering::Relaxed);
                        }
                    } else {
                        if !spawned {
                            spawned = true;
                            for w in &workers {
                                let pool = &pool;
                                thread::Builder::new()
                                    .name("mithril-worker".into())
                                    .stack_size(WORKER_STACK)
                                    .spawn_scoped(sc, move || worker_loop(pool, w))
                                    .expect("failed to spawn worker thread");
                            }
                        }
                        parallel_waves += 1;
                        {
                            let mut wv = write(&pool.wave);
                            let mut at = 0u64;
                            wv.pre = Vec::with_capacity(ranges.len() + 1);
                            for r in &ranges {
                                wv.pre.push(at);
                                at += (r.hi - r.lo).max(0) as u64;
                            }
                            wv.pre.push(at);
                            wv.ranges = mem::take(&mut ranges);
                            wv.sums = sums;
                            wv.block = (total as usize / (threads * 8)).clamp(64, 1 << 16);
                            wv.next.store(0, Ordering::Relaxed);
                        }
                        run_wave(&pool, threads, &mut ctx0);
                        if let Some(p) = lock(&pool.panic).take() {
                            panic::resume_unwind(p);
                        }
                        let mut wv = write(&pool.wave);
                        ranges = mem::take(&mut wv.ranges);
                        wv.pre.clear();
                        return_sums(&mut wv.sums, &ranges, &mut ctx0, prog);
                        continue;
                    }
                    complete_ranges(&ranges, &sums, &mut ctx0, prog);
                    continue;
                }
                let Some((rule, work)) = pick(&buckets, &recs, &costs) else { break };
                let k = rule as usize;
                let n = buckets[k].len() + recs[k].len();
                if std::env::var_os("MITHRIL_TRACE_PICK").is_some() { eprintln!("pick rule={rule} n={n} work={work} boost={boost} t={:.3}", T0.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()); }
                if threads > 1 {
                    boost = if n > prev_n || n >= threads { 1 } else { (boost * 2).min(MAX_BOOST) };
                    prev_n = n;
                }
                // a single entry cannot use the pool: run it here (waking the
                // pool would just move the work, and its caches, to another core)
                if threads == 1 || work < PAR_WORK || n < 2 {
                    ctx0.native_ready = threads == 1 || n >= 4 * threads;
                    ctx0.set_fuel(fuel.saturating_mul(boost));
                    // single-threaded drain on the coordinator; buffers keep capacity
                    let mut rx = mem::take(&mut buckets[k]);
                    let mut rr = mem::take(&mut recs[k]);
                    for e in rx.drain(..) {
                        ctx0.fire_redex(rule, e);
                    }
                    for ri in rr.drain(..) {
                        ctx0.fire_rec(rule, ri);
                    }
                    buckets[k] = rx;
                    recs[k] = rr;
                    continue;
                }
                if !spawned {
                    spawned = true;
                    for w in &workers {
                        let pool = &pool;
                        thread::Builder::new()
                            .name("mithril-worker".into())
                            .stack_size(WORKER_STACK)
                            .spawn_scoped(sc, move || worker_loop(pool, w))
                            .expect("failed to spawn worker thread");
                    }
                }
                parallel_waves += 1;
                {
                    let mut wv = write(&pool.wave);
                    wv.rule = rule;
                    mem::swap(&mut wv.redexes, &mut buckets[k]);
                    mem::swap(&mut wv.recs, &mut recs[k]);
                    let n = wv.redexes.len() + wv.recs.len();
                    wv.block = (n / (threads * 8)).clamp(1, 256);
                    wv.next.store(0, Ordering::Relaxed);
                    // Suspension exists to expose work to idle workers. With
                    // several entries per worker already, a dive gets a
                    // proportionally larger budget, so the frontier stops
                    // growing (every split costs a record and locality); a
                    // thin frontier keeps the base budget and splits often.
                    let f = wave_fuel(fuel, n, threads).saturating_mul(boost);
                    ctx0.native_ready = n >= 4 * threads;
                    ctx0.set_fuel(f);
                    for w in &workers {
                        let mut w = lock(w);
                        w.native_ready = n >= 4 * threads;
                        w.set_fuel(f);
                    }
                }
                run_wave(&pool, threads, &mut ctx0);
                if let Some(p) = lock(&pool.panic).take() {
                    panic::resume_unwind(p);
                }
                {
                    let mut wv = write(&pool.wave);
                    wv.redexes.clear();
                    wv.recs.clear();
                    mem::swap(&mut wv.redexes, &mut buckets[k]);
                    mem::swap(&mut wv.recs, &mut recs[k]);
                }
                for w in &workers {
                    lock(w).merge_into(&mut buckets, &mut recs);
                }
            }
        });

        let mut st = Stats { peak_cells: ctx0.issued, live_peak: ctx0.issued as i64, parallel_waves, rewrites: ctx0.rewrites };
        for w in workers {
            let w = w.into_inner().unwrap_or_else(PoisonError::into_inner);
            st.peak_cells += w.issued;
            st.live_peak += w.issued as i64;
            st.rewrites += w.rewrites;
        }
        self.stats = st;
        self.arena.result().expect("run finished without delivering a result to ROOT")
    }
}

/// Indices below which a range request runs on the coordinator.
const RANGE_PAR: u64 = 1 << 12;

/// Start the prepared wave on the pool, drain it here too, and wait for
/// its end: spin, then park until the last worker (which sees `waiting`)
/// notifies under the lock.
fn run_wave(pool: &Pool, threads: usize, ctx0: &mut Wctx) {
    {
        let mut c = lock(&pool.ctrl);
        c.epoch += 1;
        pool.running.store(threads - 1, Ordering::SeqCst);
        pool.epoch.store(c.epoch, Ordering::SeqCst);
    }
    if pool.parked.load(Ordering::SeqCst) > 0 {
        pool.start.notify_all();
    }
    drain(&read(&pool.wave), ctx0);
    if !spin_until(pool.spins, || pool.running.load(Ordering::Acquire) == 0) {
        let mut c = lock(&pool.ctrl);
        pool.waiting.store(true, Ordering::SeqCst);
        while pool.running.load(Ordering::SeqCst) > 0 {
            c = pool.done.wait(c).unwrap_or_else(PoisonError::into_inner);
        }
        pool.waiting.store(false, Ordering::SeqCst);
        drop(c);
    }
}

fn return_sums(sums: &mut Vec<std::sync::atomic::AtomicU64>, ranges: &[crate::RangeReq], ctx0: &mut Wctx, prog: &dyn Program) {
    let taken = mem::take(sums);
    complete_ranges(ranges, &taken, ctx0, prog);
}

/// Release each request's ports but its accumulator and deliver its result
/// to the two slots of its record (see `RangeReq`).
fn complete_ranges(ranges: &[crate::RangeReq], sums: &[std::sync::atomic::AtomicU64], ctx0: &mut Wctx, prog: &dyn Program) {
    for (q, r) in ranges.iter().enumerate() {
        let acc = r.ports[r.acc as usize];
        for (k, p) in r.ports.iter().enumerate() {
            if k != r.acc as usize {
                prog.release(*p, ctx0);
            }
        }
        let slot = (r.rec as u64) << 3;
        if r.kind == 0 {
            ctx0.deliver(slot, acc);
            ctx0.deliver(slot | 1, acc);
        } else {
            let s = sums[q].load(Ordering::Relaxed) as i64;
            ctx0.deliver(slot, crate::prelude::num(if r.kind == 2 { s & 0xffff_ffff } else { s }));
            ctx0.deliver(slot | 1, acc);
        }
    }
}

/// Per-dive budget for a parallel wave of `n` entries: the base budget
/// times the entries each worker will take (capped).
fn wave_fuel(base: i64, n: usize, threads: usize) -> i64 {
    if std::env::var_os("MITHRIL_FIXED_FUEL").is_some() {
        return base;
    }
    let per = n.div_ceil(4 * threads.max(1)).clamp(1, 1 << 10) as i64;
    base.saturating_mul(per)
}

/// Bucket with the largest `entries x cost`; `None` when all are empty.
fn pick(buckets: &[Vec<Redex>], recs: &[Vec<u32>], costs: &[u128]) -> Option<(u16, u128)> {
    let mut best: Option<(u16, u128)> = None;
    for (k, cost) in costs.iter().enumerate() {
        let n = (buckets[k].len() + recs[k].len()) as u128;
        if n > 0 && best.is_none_or(|(_, w)| n * cost > w) {
            best = Some((k as u16, n * cost));
        }
    }
    best
}

/// The wave currently being drained by the pool: a bucket's entries, or
/// the indices of range requests (`pre`: each request's first index in the
/// wave's index space; `sums`: each sum's partials).
#[derive(Default)]
struct Wave {
    rule: u16,
    redexes: Vec<Redex>,
    recs: Vec<u32>,
    ranges: Vec<crate::RangeReq>,
    pre: Vec<u64>,
    sums: Vec<std::sync::atomic::AtomicU64>,
    next: AtomicUsize,
    block: usize,
}

#[derive(Default)]
struct Ctrl {
    epoch: u64,
    quit: bool,
}

/// Waves start by bumping `epoch` (under `ctrl`, mirrored in the atomic) and
/// end when `running` reaches 0. Workers and the coordinator spin on the
/// atomics first and park on the condvars only after `SPIN`: `parked` and
/// `waiting` tell the other side a wake is needed.
#[derive(Default)]
struct Pool {
    ctrl: Mutex<Ctrl>,
    start: Condvar,
    done: Condvar,
    epoch: AtomicU64,
    running: AtomicUsize,
    parked: AtomicUsize,
    waiting: AtomicBool,
    /// `SPIN` when every thread has a physical core, else 0: a spinner on a
    /// core's second hardware thread slows the thread working beside it
    /// (8 cores x 2: heat2d at 15 and 16 threads 0.25 -> 0.42 s)
    spins: u32,
    wave: RwLock<Wave>,
    panic: Mutex<Option<Box<dyn Any + Send>>>,
}

/// Physical cores (distinct package and core ids, Linux sysfs); the
/// logical CPU count where the topology is not readable.
fn physical_cores() -> usize {
    let logical = thread::available_parallelism().map_or(1, |n| n.get());
    let mut cores = std::collections::HashSet::new();
    for cpu in 0..logical {
        let read = |f: &str| std::fs::read_to_string(format!("/sys/devices/system/cpu/cpu{cpu}/topology/{f}")).ok();
        match (read("physical_package_id"), read("core_id")) {
            (Some(p), Some(c)) => cores.insert((p.trim().to_string(), c.trim().to_string())),
            _ => return logical,
        };
    }
    cores.len().max(1)
}

/// Spin up to `spins` times for `ready`; whether it became true. Every 64th
/// spin yields the core, so a worker preempted mid-wave can run.
fn spin_until(spins: u32, ready: impl Fn() -> bool) -> bool {
    for i in 0..spins {
        if ready() {
            return true;
        }
        if i % 64 == 63 {
            thread::yield_now();
        } else {
            std::hint::spin_loop();
        }
    }
    ready()
}

/// Tells parked workers to exit when the coordinator leaves the scope,
/// including by panic, so `thread::scope` can join them.
struct QuitOnDrop<'a>(&'a Pool);
impl Drop for QuitOnDrop<'_> {
    fn drop(&mut self) {
        lock(&self.0.ctrl).quit = true;
        self.0.start.notify_all();
    }
}

/// Claim blocks of entries from the shared cursor until the wave is empty.
fn drain(wv: &Wave, ctx: &mut Wctx) {
    if !wv.ranges.is_empty() {
        return drain_ranges(wv, ctx);
    }
    let nx = wv.redexes.len();
    let n = nx + wv.recs.len();
    while let Some(range) = crate::sync::claim_block(&wv.next, wv.block, n) {
        for j in range {
            if j < nx {
                ctx.fire_redex(wv.rule, wv.redexes[j]);
            } else {
                ctx.fire_rec(wv.rule, wv.recs[j - nx]);
            }
        }
    }
}

/// Claim blocks of the range requests' indices; a block crossing a request
/// boundary runs as one native loop per request it covers.
fn drain_ranges(wv: &Wave, ctx: &mut Wctx) {
    let total = *wv.pre.last().unwrap_or(&0) as usize;
    let prog = ctx.program();
    while let Some(block) = crate::sync::claim_block(&wv.next, wv.block, total) {
        let (mut t, end) = (block.start as u64, block.end as u64);
        let mut q = wv.pre.partition_point(|&p| p <= t) - 1;
        while t < end {
            let r = &wv.ranges[q];
            let stop = end.min(wv.pre[q + 1]);
            let (lo, hi) = (r.lo + (t - wv.pre[q]) as i64, r.lo + (stop - wv.pre[q]) as i64);
            let v = prog.range_leaf(r.fid, lo, hi, &r.ports, ctx);
            if r.kind != 0 {
                wv.sums[q].fetch_add(v as u64, Ordering::Relaxed);
            }
            t = stop;
            q += 1;
        }
    }
}

fn worker_loop(pool: &Pool, me: &Mutex<Wctx>) {
    let mut seen = 0u64;
    loop {
        if spin_until(pool.spins, || pool.epoch.load(Ordering::Acquire) != seen) {
            seen = pool.epoch.load(Ordering::Acquire);
        } else {
            // announce the park before checking under the lock, so a wave
            // started meanwhile either is seen here or wakes this worker
            pool.parked.fetch_add(1, Ordering::SeqCst);
            let mut c = lock(&pool.ctrl);
            while c.epoch == seen && !c.quit {
                c = pool.start.wait(c).unwrap_or_else(PoisonError::into_inner);
            }
            pool.parked.fetch_sub(1, Ordering::SeqCst);
            if c.quit {
                return;
            }
            seen = c.epoch;
        }
        let res = {
            let mut ctx = lock(me);
            let wv = read(&pool.wave);
            panic::catch_unwind(AssertUnwindSafe(|| drain(&wv, &mut ctx)))
        };
        if let Err(p) = res {
            lock(&pool.panic).get_or_insert(p);
        }
        if pool.running.fetch_sub(1, Ordering::SeqCst) == 1 && pool.waiting.load(Ordering::SeqCst) {
            let _c = lock(&pool.ctrl);
            pool.done.notify_all();
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn read<T>(l: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    l.read().unwrap_or_else(PoisonError::into_inner)
}

fn write<T>(l: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    l.write().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod wave_fuel_tests {
    use super::wave_fuel;

    #[test]
    fn thin_frontier_keeps_the_base_budget() {
        // up to 4 entries per worker: split as often as before
        assert_eq!(wave_fuel(16384, 1, 16), 16384);
        assert_eq!(wave_fuel(16384, 16, 16), 16384);
        assert_eq!(wave_fuel(16384, 64, 16), 16384);
    }

    #[test]
    fn budget_grows_with_entries_per_worker_and_is_capped() {
        assert_eq!(wave_fuel(16384, 65, 16), 2 * 16384);
        assert_eq!(wave_fuel(16384, 64 * 8, 16), 8 * 16384);
        assert_eq!(wave_fuel(16384, usize::MAX / 2, 16), 16384 << 10);
        // no overflow on huge bases
        assert_eq!(wave_fuel(i64::MAX, 1 << 20, 16), i64::MAX);
    }
}
