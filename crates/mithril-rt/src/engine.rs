//! The scheduler: a work-stealing pool of persistent workers.
//!
//! Every `fire` buffers what it spawns; when it returns, the buffer is
//! published to the worker's own deque, so nothing a firing rule created
//! is visible to another worker before the rule has finished wiring it up
//! (record parents in particular). A worker pops its own deque from the
//! back (depth-first, cache-warm) and steals from the front of another's
//! (the oldest entry, usually the largest pending subtree). A global
//! in-flight count (published entries plus entries being fired) decides
//! termination. Single-threaded runs drain the same deque on the caller.
//!
//! Grouping ready entries by rule into barrier-separated waves only pays on
//! SIMT hardware (see the GPU crate); on the CPU a barrier per wave stalled
//! every worker on the slowest entry of each wave.

use crate::alloc::{cap_from_env, Arena};
use crate::worker::{Item, Wctx};
use crate::{Program, Redex, Stats};
use std::any::Any;
use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread;

/// Stack for pool workers (native recursion inside fuel-bounded dives).
const WORKER_STACK: usize = 16 << 20;

pub struct Engine {
    threads: usize,
    fuel: i64,
    arena: Arena,
    stats: Stats,
}

impl Engine {
    /// Engine with capacities from `MITHRIL_NODES` (default 2^26 cells) /
    /// `MITHRIL_RECS` (default 2^27 records). Both are reservations that
    /// commit memory only as chunks are used; max 2^32 each (u32 indices).
    pub fn new(threads: usize, fuel: i64) -> Engine {
        let cells = cap_from_env("MITHRIL_NODES", 1 << 26);
        let recs = cap_from_env("MITHRIL_RECS", 1 << 27);
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

        let mut ctx0 = Wctx::new(ar, prog, fuel, n_rules);
        ctx0.spawn(0, boot);

        if threads == 1 {
            let mut q: VecDeque<Item> = VecDeque::new();
            loop {
                ctx0.take_spawned(&mut q);
                let Some(it) = q.pop_back() else { break };
                ctx0.fire_item(it);
            }
            self.stats = Stats { peak_cells: ctx0.issued, live_peak: ctx0.issued as i64, pool_fired: 0, rewrites: ctx0.rewrites };
            return self.arena.result().expect("run finished without delivering a result to ROOT");
        }

        let workers: Vec<Mutex<Wctx>> = (1..threads).map(|_| Mutex::new(Wctx::new(ar, prog, fuel, n_rules))).collect();
        let pool = Pool {
            deques: (0..threads).map(|_| Mutex::new(VecDeque::new())).collect(),
            inflight: AtomicUsize::new(0),
            quit: AtomicBool::new(false),
            panic: Mutex::new(None),
        };
        publish(&pool, 0, &mut ctx0);
        thread::scope(|sc| {
            let _quit = QuitOnDrop(&pool);
            for (i, w) in workers.iter().enumerate() {
                let pool = &pool;
                thread::Builder::new()
                    .name("mithril-worker".into())
                    .stack_size(WORKER_STACK)
                    .spawn_scoped(sc, move || {
                        let mut ctx = lock(w);
                        let res = panic::catch_unwind(AssertUnwindSafe(|| work_loop(pool, i + 1, threads, &mut ctx)));
                        if let Err(p) = res {
                            lock(&pool.panic).get_or_insert(p);
                            pool.quit.store(true, Ordering::Release);
                        }
                    })
                    .expect("failed to spawn worker thread");
            }
            work_loop(&pool, 0, threads, &mut ctx0);
        });
        if let Some(p) = lock(&pool.panic).take() {
            panic::resume_unwind(p);
        }

        let mut st = Stats { peak_cells: ctx0.issued, live_peak: ctx0.issued as i64, pool_fired: 0, rewrites: ctx0.rewrites };
        for w in workers {
            let w = w.into_inner().unwrap_or_else(PoisonError::into_inner);
            st.peak_cells += w.issued;
            st.live_peak += w.issued as i64;
            st.rewrites += w.rewrites;
            st.pool_fired += w.rewrites;
        }
        self.stats = st;
        self.arena.result().expect("run finished without delivering a result to ROOT")
    }
}

struct Pool {
    deques: Vec<Mutex<VecDeque<Item>>>,
    /// Published entries not yet fired plus entries being fired right now.
    inflight: AtomicUsize,
    quit: AtomicBool,
    panic: Mutex<Option<Box<dyn Any + Send>>>,
}

/// Tells workers to exit when the coordinator leaves the scope, including
/// by panic, so `thread::scope` can join them.
struct QuitOnDrop<'a>(&'a Pool);
impl Drop for QuitOnDrop<'_> {
    fn drop(&mut self) {
        self.0.quit.store(true, Ordering::Release);
    }
}

/// Move everything `ctx` spawned during the last fire onto deque `me`.
/// Counted into `inflight` before the firing entry is counted out, so the
/// count never reads zero while work exists.
fn publish(pool: &Pool, me: usize, ctx: &mut Wctx) {
    let mut dq = lock(&pool.deques[me]);
    let before = dq.len();
    ctx.take_spawned(&mut dq);
    let n = dq.len() - before;
    if n > 0 {
        pool.inflight.fetch_add(n, Ordering::AcqRel);
    }
}

fn work_loop(pool: &Pool, me: usize, threads: usize, ctx: &mut Wctx) {
    let mut idle = 0u32;
    loop {
        if pool.quit.load(Ordering::Acquire) {
            return;
        }
        let mine = lock(&pool.deques[me]).pop_back();
        let item = match mine {
            Some(it) => it,
            None => {
                let mut stolen = None;
                for k in 1..threads {
                    if let Some(it) = lock(&pool.deques[(me + k) % threads]).pop_front() {
                        stolen = Some(it);
                        break;
                    }
                }
                match stolen {
                    Some(it) => it,
                    None => {
                        if pool.inflight.load(Ordering::Acquire) == 0 {
                            return;
                        }
                        idle += 1;
                        if idle < 128 {
                            std::hint::spin_loop();
                        } else {
                            thread::yield_now();
                        }
                        continue;
                    }
                }
            }
        };
        idle = 0;
        ctx.fire_item(item);
        publish(pool, me, ctx);
        pool.inflight.fetch_sub(1, Ordering::AcqRel);
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
