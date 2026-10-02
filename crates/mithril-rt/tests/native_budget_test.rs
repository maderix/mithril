use mithril_rt::prelude::native_work_fuel;

#[test]
fn native_work_is_charged_without_truncating_or_saturating() {
    for (start, charge, want) in [
        (100, 0, 100),
        (100, 99, 1),
        (100, 101, -1),
        (100, 4294967297, -4294967197),
        (i64::MIN + 4, 13, i64::MAX - 8),
    ] {
        let mut fuel = start;
        native_work_fuel(&mut fuel, charge);
        assert_eq!(fuel, want);
    }
}

#[test]
fn deferred_delivery_allows_the_caller_to_attach_a_completed_chain() {
    use mithril_rt::{DiveResult, Engine, Program, Redex, Wctx, ROOT};
    struct Chain;
    impl Program for Chain {
        fn n_rules(&self) -> usize { 3 }
        fn rule_cost(&self, _: u16) -> u32 { 1 }
        fn fire(&self, rule: u16, e: Redex, ctx: &mut Wctx) {
            if rule == 0 {
                let base = ctx.alloc_rec(1, 1, 0, 0, u64::MAX);
                ctx.deliver_deferred((base as u64) << 3, 5);
                let mut tail = base;
                for _ in 0..e.a {
                    let join = ctx.alloc_rec(2, 2, 0, 0, u64::MAX);
                    ctx.set_parent(tail, (join as u64) << 3);
                    ctx.deliver_deferred(((join as u64) << 3) | 1, 7);
                    tail = join;
                }
                ctx.set_parent(tail, ROOT);
            } else {
                let parent = ctx.rec(e.aux as u32).parent;
                ctx.deliver(parent, if rule == 1 { e.a } else { e.a + e.b });
            }
        }
        fn dive(&self, _: u16, _: &[u64], _: &mut i64, _: &mut Wctx) -> DiveResult { unreachable!() }
    }
    for threads in [1, 4] {
        for (depth, want) in [(0, 5), (1, 12), (64, 453), (257, 1804)] {
            let mut engine = Engine::with_capacity(threads, 1, 4096, 4096);
            assert_eq!(engine.run(&Chain, Redex { a: depth, b: 0, aux: 0 }), want);
        }
    }
}
