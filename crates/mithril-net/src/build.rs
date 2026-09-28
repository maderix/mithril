//! Net construction. `build` creates the initial net: the root wire (cell
//! 0) and one redex pairing `main` as a `Ref` against the output var.
//! `instantiate` splices an entry's body into a live net during `Ref`
//! unfolding — this is also where DUP chains are inserted for every
//! variable used more than once.

use crate::{instantiate, list_alloc, ref_port, wire, NetProg, EMPTY};
use mithril_core::net::Net;
use mithril_core::port::{Port, Tag};
use mithril_front::core::CoreModule;

/// The output var: one end of the root wire, which `build` allocates as
/// cell 0. `readback(&net, root_port())` reads the program's result.
pub fn root_port() -> Port {
    Port::new(Tag::Var, 0)
}

/// Initial net: `main` as a REF against the output var. If `main` has
/// parameters they become fresh unfilled wires, so reduction specializes
/// the body over unknown inputs instead of evaluating it.
pub fn build(m: &CoreModule) -> Net {
    let prog = NetProg::new(m);
    let mut net = Net::new();
    let root = net.alloc(EMPTY, EMPTY);
    debug_assert_eq!(root, 0, "root wire must be cell 0");
    let main = m.main as usize;
    let arity = prog.entries[main].params.len();
    let args: Vec<Port> = (0..arity).map(|_| wire(&mut net)).collect();
    let head = list_alloc(&mut net, &args);
    net.redexes.push((ref_port(head, main as u16), root_port()));
    net
}

/// A specialization net for real function `fid`: its parameters as fresh
/// unfilled wires (returned in order), its body instantiated against the
/// root wire (cell 0).
pub(crate) fn build_fn(net: &mut Net, prog: &NetProg, fid: usize) -> Vec<Port> {
    let root = net.alloc(EMPTY, EMPTY);
    debug_assert_eq!(root, 0, "root wire must be cell 0");
    let arity = prog.entries[fid].params.len();
    let args: Vec<Port> = (0..arity).map(|_| wire(net)).collect();
    instantiate(net, &prog.entries, fid, args.clone(), root_port());
    args
}
