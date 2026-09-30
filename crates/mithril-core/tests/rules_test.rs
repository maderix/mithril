//! Rule-table unit tests over a hand-built `Net`.

use mithril_core::agents::*;
use mithril_core::net::Net;
use mithril_core::port::{Port, Tag};
use mithril_core::rules::{link, process, MatMeta, Prog};

/// A program whose calls must never unfold.
struct NoUnfold;
impl Prog<Net> for NoUnfold {
    fn unfold(&self, _: &mut Net, _: Port, _: Port) -> u64 { panic!("unfolded a call whose result is erased") }
    fn is_closure(&self, _: Port) -> bool { false }
    fn mat_meta(&self, _: u16) -> MatMeta<'_> { MatMeta::Proj(0) }
    fn compute(&self, _: &mut Net, _: u16, _: Port, _: Port) -> Option<Port> { None }
    fn park_op(&self, _: &mut Net, _: Port, _: Port) {}
}

/// A call (with an argument) whose result wire already holds ERA, through
/// two filled wires, is erased: its arguments too, and every cell freed.
#[test]
fn a_call_whose_result_is_erased_is_erased_not_unfolded() {
    let mut net = Net::new();
    let (w1, w2) = (wire(&mut net), wire(&mut net));
    link(&mut net, w1, w2); // w1's cell now holds w2 (a filled wire)
    link(&mut net, era(), Port::new(Tag::Var, w2.payload())); // w2 holds ERA
    let arg = list_alloc(&mut net, &[Port::num(7)]);
    let r = ref_port(arg, 0);
    assert_eq!(process(&mut net, &NoUnfold, r, Port::new(Tag::Var, w1.payload())), 1);
    while let Some((a, b)) = net.redexes.pop() {
        process(&mut net, &NoUnfold, a, b);
    }
    assert_eq!(net.live().count(), 0, "cells left: {}", net.dump());
}
