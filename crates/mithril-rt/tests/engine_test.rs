//! Engine tests. Every Program here is a test-only toy written against the
//! public `Program`/`Wctx` API, exactly as generated code would be.

use mithril_rt::{DiveResult, Engine, Program, Redex, Wctx, ROOT};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Toy 1: fork tree. fork(n) = 1 if n == 0 else fork(n-1) + fork(n-1).
// ---------------------------------------------------------------------------

const FORK: u16 = 0;
const JOIN: u16 = 1;

/// `cut`: fork(n) with n <= cut runs the fuel-bounded dive instead of
/// splitting into records. `cut = 0` exercises the pure rule form.
struct ForkTree {
    cut: u64,
}

impl ForkTree {
    /// Sequential fold over the fork tree with an explicit stack (so the dive
    /// itself never recurses natively). Each node costs one fuel unit; on
    /// exhaustion the pending frames become JOIN records and the unevaluated
    /// subtrees become FORK redexes.
    fn dive_fork(&self, n0: u64, parent: u64, fuel: &mut i64, ctx: &mut Wctx) -> DiveResult {
        // (n, left value once computed)
        let mut stack: Vec<(u64, Option<u64>)> = Vec::new();
        let mut cur = n0;
        loop {
            *fuel -= 1;
            if *fuel < 0 {
                let mut dest = parent;
                for &(n, left) in &stack {
                    let j = ctx.alloc_rec(JOIN, 2, 0, 0, dest);
                    let jp = (j as u64) << 3;
                    match left {
                        // evaluating the left child: right subtree becomes a redex
                        None => {
                            ctx.spawn(FORK, Redex { a: n - 1, b: jp | 1, aux: 0 });
                            dest = jp;
                        }
                        // evaluating the right child: left value is already known
                        Some(lv) => {
                            ctx.deliver(jp, lv);
                            dest = jp | 1;
                        }
                    }
                }
                ctx.spawn(FORK, Redex { a: cur, b: dest, aux: 0 });
                return DiveResult::Suspended;
            }
            if cur > 0 {
                stack.push((cur, None));
                cur -= 1;
                continue;
            }
            let mut v = 1u64;
            loop {
                match stack.last_mut() {
                    None => return DiveResult::Done(v),
                    Some((n, l @ None)) => {
                        *l = Some(v);
                        cur = *n - 1;
                        break;
                    }
                    Some((_, Some(lv))) => {
                        v += *lv;
                        stack.pop();
                    }
                }
            }
        }
    }
}

impl Program for ForkTree {
    fn n_rules(&self) -> usize {
        2
    }
    fn rule_cost(&self, rule: u16) -> u32 {
        match rule {
            FORK => 4,
            _ => 1,
        }
    }
    fn fire(&self, rule: u16, e: Redex, ctx: &mut Wctx) {
        match rule {
            FORK => {
                let (n, parent) = (e.a, e.b);
                if n <= self.cut {
                    if let DiveResult::Done(v) = ctx.dive(0, &[n, parent]) {
                        ctx.deliver(parent, v);
                    }
                } else {
                    let j = ctx.alloc_rec(JOIN, 2, 0, 0, parent);
                    let jp = (j as u64) << 3;
                    ctx.spawn(FORK, Redex { a: n - 1, b: jp, aux: 0 });
                    ctx.spawn(FORK, Redex { a: n - 1, b: jp | 1, aux: 0 });
                }
            }
            JOIN => {
                let parent = ctx.rec(e.aux as u32).parent;
                ctx.deliver(parent, e.a + e.b);
            }
            _ => unreachable!(),
        }
    }
    fn dive(&self, f: u16, args: &[u64], fuel: &mut i64, ctx: &mut Wctx) -> DiveResult {
        assert_eq!(f, 0);
        self.dive_fork(args[0], args[1], fuel, ctx)
    }
}

fn fork_boot(n: u64) -> Redex {
    Redex { a: n, b: ROOT, aux: 0 }
}

fn small_engine(threads: usize, fuel: i64) -> Engine {
    Engine::with_capacity(threads, fuel, 1 << 22, 1 << 22)
}

#[test]
fn engine_empty_program() {
    struct Imm;
    impl Program for Imm {
        fn n_rules(&self) -> usize {
            1
        }
        fn rule_cost(&self, _: u16) -> u32 {
            1
        }
        fn fire(&self, rule: u16, e: Redex, ctx: &mut Wctx) {
            assert_eq!(rule, 0);
            ctx.deliver(e.b, e.a);
        }
        fn dive(&self, _: u16, _: &[u64], _: &mut i64, _: &mut Wctx) -> DiveResult {
            unreachable!()
        }
    }
    for threads in [1, 8] {
        let mut eng = small_engine(threads, 1000);
        let r = eng.run(&Imm, Redex { a: 42, b: ROOT, aux: 0 });
        assert_eq!(r, 42);
        let st = eng.stats();
        assert_eq!(st.rewrites, 1);
        assert_eq!(st.parallel_waves, 0);
        assert_eq!(st.peak_cells, 0);
    }
}

#[test]
fn fork_tree_20_threads_1_and_8() {
    // Default-capacity constructor (env-driven), as generated mains use.
    let prog = ForkTree { cut: 0 };
    let r1 = Engine::new(1, 1 << 11).run(&prog, fork_boot(20));
    let r8 = Engine::new(8, 1 << 11).run(&prog, fork_boot(20));
    assert_eq!(r1, 1 << 20);
    assert_eq!(r8, r1);
}

#[test]
fn fork_tree_rule_form_uses_parallel_waves() {
    let prog = ForkTree { cut: 0 };
    let mut eng = small_engine(8, 1 << 11);
    assert_eq!(eng.run(&prog, fork_boot(20)), 1 << 20);
    let st = eng.stats();
    assert!(st.parallel_waves > 0, "wide fork waves must drain in parallel");
    // 2^21 - 1 forks + 2^20 - 1 joins
    assert_eq!(st.rewrites, (1u64 << 21) - 1 + (1u64 << 20) - 1);
    // single thread never runs a parallel wave
    let mut eng1 = small_engine(1, 1 << 11);
    assert_eq!(eng1.run(&prog, fork_boot(20)), 1 << 20);
    assert_eq!(eng1.stats().parallel_waves, 0);
}

#[test]
fn fork_tree_mixed_dive_and_rules_all_fuels() {
    // cut=10: subtrees of 2^11 - 1 nodes dive; fuel below that forces
    // suspension mid-dive, fuel above it completes every dive.
    for cut in [5u64, 10, 16] {
        let prog = ForkTree { cut };
        for fuel in [1i64, 3, 7, 100, 1000, 1 << 20] {
            let r1 = small_engine(1, fuel).run(&prog, fork_boot(16));
            let r8 = small_engine(8, fuel).run(&prog, fork_boot(16));
            assert_eq!(r1, 1 << 16, "cut={cut} fuel={fuel}");
            assert_eq!(r8, r1, "cut={cut} fuel={fuel}");
        }
    }
}

#[test]
fn full_dive_no_suspension_single_rewrite() {
    // Whole tree in one dive: exactly one rewrite, no records left behind.
    let prog = ForkTree { cut: 64 };
    let mut eng = small_engine(4, 1 << 30);
    assert_eq!(eng.run(&prog, fork_boot(18)), 1 << 18);
    assert_eq!(eng.stats().rewrites, 1);
}

#[test]
fn fuel_nonpositive_is_clamped_to_progress() {
    // Fuel 0 / negative must not livelock: the engine grants at least 1 unit.
    let prog = ForkTree { cut: 64 };
    for fuel in [0i64, -5] {
        assert_eq!(small_engine(2, fuel).run(&prog, fork_boot(8)), 1 << 8);
    }
}

#[test]
fn engine_is_reusable_across_runs() {
    let prog = ForkTree { cut: 4 };
    let mut eng = small_engine(8, 50);
    let a = eng.run(&prog, fork_boot(14));
    let sa = eng.stats();
    let b = eng.run(&prog, fork_boot(14));
    let sb = eng.stats();
    assert_eq!(a, 1 << 14);
    assert_eq!(a, b);
    // stats are per run, not cumulative
    assert_eq!(sa.rewrites, sb.rewrites);
    assert_eq!(eng.run(&prog, fork_boot(3)), 8);
}

// ---------------------------------------------------------------------------
// Toy 2: deep right-nested list. build(n) = Cons(n, build(n-1)) (non-tail,
// 10^6 deep), then sum(list) consuming and freeing every cell. Repeats for
// `rounds` rounds with an accumulated total. rounds == 0 delivers the list.
// ---------------------------------------------------------------------------

const L_MAIN: u16 = 0; // {a: n, b: parent, aux: rounds}
const L_BUILD: u16 = 1; // {a: n, b: dest}
const L_CONS: u16 = 2; // record: a = tail, d = head
const L_THEN: u16 = 3; // record: a = list, b = acc, d = n, s = rounds left
const L_SUM: u16 = 4; // {a: list, b: dest, aux: acc}
const L_AFTER: u16 = 5; // record: a = total, d = n, s = rounds left
const NIL: u64 = u64::MAX;

struct DeepList;

impl DeepList {
    fn start_round(ctx: &mut Wctx, n: u64, rounds: u32, acc: u64, parent: u64) {
        let t = ctx.alloc_rec(L_THEN, 2, n as u32, rounds, parent);
        ctx.deliver((t as u64) << 3 | 1, acc);
        ctx.spawn(L_BUILD, Redex { a: n, b: (t as u64) << 3, aux: 0 });
    }

    /// build(n) with fuel: descend collecting heads, then cons bottom-up.
    fn build(n: u64, dest: u64, fuel: &mut i64, ctx: &mut Wctx) -> DiveResult {
        let mut heads = Vec::new();
        let mut m = n;
        loop {
            *fuel -= 1;
            if *fuel < 0 {
                // pending cons frames become a chain of CONS records
                let mut d = dest;
                for &h in &heads {
                    let r = ctx.alloc_rec(L_CONS, 1, h as u32, 0, d);
                    d = (r as u64) << 3;
                }
                ctx.spawn(L_BUILD, Redex { a: m, b: d, aux: 0 });
                return DiveResult::Suspended;
            }
            if m == 0 {
                break;
            }
            heads.push(m);
            m -= 1;
        }
        let mut v = NIL;
        for &h in heads.iter().rev() {
            v = ctx.alloc(h, v) as u64;
        }
        DiveResult::Done(v)
    }

    /// Tail-recursive sum loop with fuel; frees every consumed cell.
    fn sum(mut list: u64, dest: u64, mut acc: u64, fuel: &mut i64, ctx: &mut Wctx) -> DiveResult {
        while list != NIL {
            *fuel -= 1;
            if *fuel < 0 {
                ctx.spawn(L_SUM, Redex { a: list, b: dest, aux: acc });
                return DiveResult::Suspended;
            }
            let [h, t] = ctx.cell(list as u32);
            ctx.free(list as u32);
            acc += h;
            list = t;
        }
        DiveResult::Done(acc)
    }
}

impl Program for DeepList {
    fn n_rules(&self) -> usize {
        6
    }
    fn rule_cost(&self, _: u16) -> u32 {
        1
    }
    fn fire(&self, rule: u16, e: Redex, ctx: &mut Wctx) {
        match rule {
            L_MAIN => {
                if e.aux == 0 {
                    ctx.spawn(L_BUILD, Redex { a: e.a, b: e.b, aux: 0 });
                } else {
                    Self::start_round(ctx, e.a, e.aux as u32, 0, e.b);
                }
            }
            L_BUILD => {
                if let DiveResult::Done(v) = ctx.dive(0, &[e.a, e.b]) {
                    ctx.deliver(e.b, v);
                }
            }
            L_CONS => {
                let r = ctx.rec(e.aux as u32);
                let c = ctx.alloc(r.d as u64, e.a);
                ctx.deliver(r.parent, c as u64);
            }
            L_THEN => {
                let r = ctx.rec(e.aux as u32);
                let after = ctx.alloc_rec(L_AFTER, 1, r.d, r.s, r.parent);
                ctx.spawn(L_SUM, Redex { a: e.a, b: (after as u64) << 3, aux: e.b });
            }
            L_SUM => {
                if let DiveResult::Done(v) = ctx.dive(1, &[e.a, e.b, e.aux]) {
                    ctx.deliver(e.b, v);
                }
            }
            L_AFTER => {
                let r = ctx.rec(e.aux as u32);
                if r.s <= 1 {
                    ctx.deliver(r.parent, e.a);
                } else {
                    Self::start_round(ctx, r.d as u64, r.s - 1, e.a, r.parent);
                }
            }
            _ => unreachable!(),
        }
    }
    fn dive(&self, f: u16, args: &[u64], fuel: &mut i64, ctx: &mut Wctx) -> DiveResult {
        match f {
            0 => Self::build(args[0], args[1], fuel, ctx),
            1 => Self::sum(args[0], args[1], args[2], fuel, ctx),
            _ => unreachable!(),
        }
    }
}

#[test]
fn dive_suspend_resume_deep_list() {
    let n: u64 = 1_000_000;
    let boot = Redex { a: n, b: ROOT, aux: 1 };
    let mut e1 = Engine::with_capacity(1, 1000, 1 << 21, 1 << 21);
    let mut e8 = Engine::with_capacity(8, 1000, 1 << 21, 1 << 21);
    let r1 = e1.run(&DeepList, boot);
    let r8 = e8.run(&DeepList, boot);
    assert_eq!(r1, n * (n + 1) / 2);
    assert_eq!(r8, r1);
    // Fuel 1000 really suspended: ~1000 BUILD + ~1000 SUM resumptions and a
    // 10^6-long chain of CONS records, each activated by a non-recursive deliver.
    for st in [e1.stats(), e8.stats()] {
        assert!(st.rewrites > n + 1900, "rewrites {}", st.rewrites);
        assert_eq!(st.peak_cells, n as usize);
    }
}

#[test]
fn deep_list_readback_through_engine_cell() {
    let n: u64 = 100_000;
    for threads in [1, 8] {
        let mut eng = Engine::with_capacity(threads, 777, 1 << 20, 1 << 20);
        let mut p = eng.run(&DeepList, Redex { a: n, b: ROOT, aux: 0 });
        let mut expect = n;
        while p != NIL {
            let [h, t] = eng.cell(p as u32);
            assert_eq!(h, expect);
            expect -= 1;
            p = t;
        }
        assert_eq!(expect, 0);
        assert_eq!(eng.stats().peak_cells, n as usize);
    }
}

#[test]
fn free_list_reuse_bounded() {
    // 6 rounds of build+consume n cells: live cells never exceed n, so the
    // footprint must stay well below the 6n a leaking allocator would need.
    let n: u64 = 100_000;
    let rounds = 6u64;
    let expect = rounds * n * (n + 1) / 2;
    for threads in [1, 8] {
        let mut eng = Engine::with_capacity(threads, 1000, 1 << 20, 1 << 20);
        let r = eng.run(&DeepList, Redex { a: n, b: ROOT, aux: rounds });
        assert_eq!(r, expect);
        let peak = eng.stats().peak_cells;
        assert!(peak >= n as usize, "peak {peak} below live set");
        assert!(peak < 4 * n as usize, "peak {peak} >= 4x live {n}");
    }
}

// ---------------------------------------------------------------------------
// Toy 3: parallel cell churn. Leaves allocate a cell, joins read both
// children, free them and allocate the sum cell. Checks reuse across workers.
// ---------------------------------------------------------------------------

struct CellTree;
impl Program for CellTree {
    fn n_rules(&self) -> usize {
        2
    }
    fn rule_cost(&self, _: u16) -> u32 {
        8
    }
    fn fire(&self, rule: u16, e: Redex, ctx: &mut Wctx) {
        match rule {
            0 => {
                if e.a == 0 {
                    let c = ctx.alloc(1, 0);
                    ctx.deliver(e.b, c as u64);
                } else {
                    let j = ctx.alloc_rec(1, 2, 0, 0, e.b);
                    ctx.spawn(0, Redex { a: e.a - 1, b: (j as u64) << 3, aux: 0 });
                    ctx.spawn(0, Redex { a: e.a - 1, b: (j as u64) << 3 | 1, aux: 0 });
                }
            }
            1 => {
                let parent = ctx.rec(e.aux as u32).parent;
                let (x, y) = (ctx.cell(e.a as u32)[0], ctx.cell(e.b as u32)[0]);
                ctx.free(e.a as u32);
                ctx.free(e.b as u32);
                let c = ctx.alloc(0, 0);
                ctx.set(c, 0, x + y);
                ctx.set(c, 1, 7);
                if parent == ROOT {
                    ctx.deliver(parent, x + y);
                } else {
                    ctx.deliver(parent, c as u64);
                }
            }
            _ => unreachable!(),
        }
    }
    fn dive(&self, _: u16, _: &[u64], _: &mut i64, _: &mut Wctx) -> DiveResult {
        unreachable!()
    }
}

#[test]
fn parallel_cell_churn_bounded_and_correct() {
    let n = 18u64;
    let leaves = 1usize << n;
    for threads in [1, 3, 8] {
        let mut eng = small_engine(threads, 1000);
        assert_eq!(eng.run(&CellTree, Redex { a: n, b: ROOT, aux: 0 }), leaves as u64);
        let peak = eng.stats().peak_cells;
        assert!(peak >= leaves, "threads={threads} peak={peak}");
        assert!(peak < 4 * leaves, "threads={threads} peak={peak}");
    }
}

// ---------------------------------------------------------------------------
// Toy 4: work-weighted scheduling.
// ---------------------------------------------------------------------------

const W_BOOT: u16 = 0;
const W_HEAVY: u16 = 1;
const W_LIGHT: u16 = 2;
const W_JOIN: u16 = 3;

/// Boot spawns `heavy` HEAVY leaves and `light` LIGHT leaves, all joined by
/// a chain of JOIN records into one sum. HEAVY delivers i*i, LIGHT 1.
struct Weighted {
    heavy: u64,
    light: u64,
    heavy_cost: u32,
    order: Mutex<Vec<u16>>,
    busy: AtomicU64,
}

impl Weighted {
    fn new(heavy: u64, light: u64, heavy_cost: u32) -> Weighted {
        Weighted { heavy, light, heavy_cost, order: Mutex::new(Vec::new()), busy: AtomicU64::new(0) }
    }
}

impl Program for Weighted {
    fn n_rules(&self) -> usize {
        4
    }
    fn rule_cost(&self, rule: u16) -> u32 {
        match rule {
            W_HEAVY => self.heavy_cost,
            _ => 1,
        }
    }
    fn fire(&self, rule: u16, e: Redex, ctx: &mut Wctx) {
        self.order.lock().unwrap().push(rule);
        match rule {
            W_BOOT => {
                let leaves: Vec<(u16, u64)> = (0..self.heavy)
                    .map(|i| (W_HEAVY, i))
                    .chain((0..self.light).map(|i| (W_LIGHT, i)))
                    .collect();
                // chain: j_k.0 <- leaf k, j_k.1 <- j_{k+1}; last join takes two leaves
                let mut dest = e.b;
                for (k, &(r, i)) in leaves.iter().enumerate() {
                    if k + 1 == leaves.len() {
                        ctx.spawn(r, Redex { a: i, b: dest, aux: 0 });
                    } else {
                        let j = ctx.alloc_rec(W_JOIN, 2, 0, 0, dest);
                        ctx.spawn(r, Redex { a: i, b: (j as u64) << 3, aux: 0 });
                        dest = (j as u64) << 3 | 1;
                    }
                }
            }
            W_HEAVY => {
                let mut x = e.a;
                for _ in 0..(1 << 12) {
                    x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
                }
                self.busy.fetch_add(x & 1, Ordering::Relaxed);
                ctx.deliver(e.b, e.a * e.a);
            }
            W_LIGHT => ctx.deliver(e.b, 1),
            W_JOIN => {
                let parent = ctx.rec(e.aux as u32).parent;
                ctx.deliver(parent, e.a + e.b);
            }
            _ => unreachable!(),
        }
    }
    fn dive(&self, _: u16, _: &[u64], _: &mut i64, _: &mut Wctx) -> DiveResult {
        unreachable!()
    }
}

#[test]
fn work_weighted_pick_drains_parallel() {
    // 10 entries x cost 2^12 = 40960 >= 2^14: must drain in parallel.
    let prog = Weighted::new(10, 0, 1 << 12);
    let mut eng = small_engine(8, 1000);
    assert_eq!(eng.run(&prog, Redex { a: 0, b: ROOT, aux: 0 }), 285);
    assert!(eng.stats().parallel_waves > 0);
}

#[test]
fn small_work_drains_single_threaded() {
    // 10 entries x cost 1 is far below 2^14: no parallel wave.
    let prog = Weighted::new(10, 0, 1);
    let mut eng = small_engine(8, 1000);
    assert_eq!(eng.run(&prog, Redex { a: 0, b: ROOT, aux: 0 }), 285);
    assert_eq!(eng.stats().parallel_waves, 0);
}

#[test]
fn threshold_boundary() {
    // 4 x 2^12 = 2^14 exactly: parallel (>=); 4 x (2^12 - 1): not.
    let at = Weighted::new(4, 0, 1 << 12);
    let mut eng = small_engine(4, 1000);
    assert_eq!(eng.run(&at, Redex { a: 0, b: ROOT, aux: 0 }), 14);
    assert_eq!(eng.stats().parallel_waves, 1);
    let below = Weighted::new(4, 0, (1 << 12) - 1);
    let mut eng = small_engine(4, 1000);
    assert_eq!(eng.run(&below, Redex { a: 0, b: ROOT, aux: 0 }), 14);
    assert_eq!(eng.stats().parallel_waves, 0);
}

#[test]
fn scheduler_prefers_weighted_work_over_entry_count() {
    // 100 LIGHT (100 x 1) vs 10 HEAVY (10 x 2^12): HEAVY bucket goes first
    // although it has fewer entries.
    let prog = Weighted::new(10, 100, 1 << 12);
    let mut eng = small_engine(4, 1000);
    assert_eq!(eng.run(&prog, Redex { a: 0, b: ROOT, aux: 0 }), 285 + 100);
    let order = prog.order.lock().unwrap();
    assert_eq!(order[0], W_BOOT);
    assert!(order[1..11].iter().all(|&r| r == W_HEAVY), "{:?}", &order[..12]);
    assert_eq!(order.iter().filter(|&&r| r == W_LIGHT).count(), 100);
    assert_eq!(order.iter().filter(|&&r| r == W_JOIN).count(), 109);
}

// ---------------------------------------------------------------------------
// Failure modes.
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "arena exhausted")]
fn cell_arena_exhaustion_panics_single_thread() {
    let mut eng = Engine::with_capacity(1, 1000, 1 << 10, 1 << 16);
    eng.run(&DeepList, Redex { a: 5000, b: ROOT, aux: 0 });
}

#[test]
#[should_panic(expected = "arena exhausted")]
fn record_arena_exhaustion_panics_in_parallel_wave() {
    // Rule-form fork tree needs ~2^16 records; cap far lower.
    let mut eng = Engine::with_capacity(8, 1000, 1 << 16, 1 << 12);
    eng.run(&ForkTree { cut: 0 }, fork_boot(16));
}

#[test]
#[should_panic(expected = "without delivering")]
fn run_without_result_panics() {
    struct Mute;
    impl Program for Mute {
        fn n_rules(&self) -> usize {
            1
        }
        fn rule_cost(&self, _: u16) -> u32 {
            1
        }
        fn fire(&self, _: u16, _: Redex, _: &mut Wctx) {}
        fn dive(&self, _: u16, _: &[u64], _: &mut i64, _: &mut Wctx) -> DiveResult {
            unreachable!()
        }
    }
    small_engine(2, 10).run(&Mute, Redex { a: 0, b: 0, aux: 0 });
}

#[test]
#[should_panic(expected = "rule boom")]
fn program_panic_in_worker_propagates() {
    struct Boom;
    impl Program for Boom {
        fn n_rules(&self) -> usize {
            2
        }
        fn rule_cost(&self, _: u16) -> u32 {
            1 << 14
        }
        fn fire(&self, rule: u16, e: Redex, ctx: &mut Wctx) {
            if rule == 0 {
                for i in 0..64 {
                    ctx.spawn(1, Redex { a: i, b: e.b, aux: 0 });
                }
            } else if e.a == 63 {
                panic!("rule boom");
            }
        }
        fn dive(&self, _: u16, _: &[u64], _: &mut i64, _: &mut Wctx) -> DiveResult {
            unreachable!()
        }
    }
    small_engine(8, 10).run(&Boom, Redex { a: 0, b: ROOT, aux: 0 });
}
