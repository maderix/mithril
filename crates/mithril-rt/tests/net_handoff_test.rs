//! A task must not end with net work of its own. Compiled code that waits on
//! a wire links a waiter into it after its last net reduction; when another
//! worker fills the wire at that moment, the link finds the value and queues
//! the pair on this worker, where nothing would reduce it: the run ended
//! without a result (closure_split under load, about 1 run in 200). The pair
//! now goes to the net rule, which fires it in a later wave.

use mithril_rt::mithril_core::port::Port;
use mithril_rt::{DiveResult, Engine, Program, Redex, Wctx, ROOT};

struct Leftover;

impl Program for Leftover {
    fn n_rules(&self) -> usize {
        2
    }
    fn net_rule(&self) -> u16 {
        1
    }
    fn rule_cost(&self, _: u16) -> u32 {
        1
    }
    fn fire(&self, rule: u16, e: Redex, ctx: &mut Wctx) {
        if rule == 0 {
            // the pair a late link queued: this task never reduces it
            ctx.net_push(Port(40 + e.a), Port(2));
        } else {
            ctx.deliver(ROOT, e.a + e.b);
        }
    }
    fn dive(&self, _: u16, _: &[u64], _: &mut i64, _: &mut Wctx) -> DiveResult {
        unreachable!()
    }
}

#[test]
fn net_work_left_by_a_task_goes_to_the_net_rule() {
    for threads in [1, 2, 4, 16] {
        let mut engine = Engine::with_capacity(threads, 1, 4096, 4096);
        assert_eq!(engine.run(&Leftover, Redex { a: 0, b: 0, aux: 0 }), 42, "threads {threads}");
    }
}
