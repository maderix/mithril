//! Net construction. `build` creates the initial net: the root wire (cell
//! 0) and one redex pairing `main` as a `Ref` against the output var.
//! `instantiate` splices an entry's body into a live net during `Ref`
//! unfolding — this is also where DUP chains are inserted for every
//! variable used more than once.

use crate::rules::link;
use crate::{
    con_alloc, dup_port, era, list_alloc, mat_port, op_port, ref_port, wire, ClosureSpec, NExpr,
    NetProg, CTAG_TUPLE, EMPTY,
};
use mithril_core::net::Net;
use mithril_core::port::{Port, Tag};
use mithril_front::core::CoreModule;
use std::collections::HashMap;
use std::collections::VecDeque;

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

/// Per-instantiation variable environment: each bound variable owns a
/// queue of use-ports (length = its statically counted use count), popped
/// front-first as occurrences are reached left-to-right.
#[derive(Default)]
pub(crate) struct Env(HashMap<u32, VecDeque<Port>>);

impl Env {
    fn give(&mut self, v: u32, ports: Vec<Port>) {
        self.0.entry(v).or_default().extend(ports);
    }
    fn take(&mut self, v: u32) -> Port {
        self.0
            .get_mut(&v)
            .and_then(|q| q.pop_front())
            .unwrap_or_else(|| panic!("ICE: use of var v{} exceeds its counted uses", v))
    }
}

/// Number of times `v` occurs in `e`, mirroring exactly how `inst`
/// consumes env ports: a deferred If branch / Match arm counts as one use
/// per branch that captures `v` (its internal occurrences get their own
/// DUP chain when the branch entry is instantiated).
pub(crate) fn count_uses(v: u32, e: &NExpr) -> usize {
    match e {
        NExpr::Num(_) | NExpr::Flo(_) => 0,
        NExpr::Var(u) => (*u == v) as usize,
        NExpr::Op2(_, a, b) => count_uses(v, a) + count_uses(v, b),
        NExpr::Let(u, r, b) => count_uses(v, r) + if *u == v { 0 } else { count_uses(v, b) },
        NExpr::Call(_, args) | NExpr::Ctor(_, args) | NExpr::Tuple(args) => {
            args.iter().map(|a| count_uses(v, a)).sum()
        }
        NExpr::If(c, t, e2) => {
            count_uses(v, c) + t.caps.contains(&v) as usize + e2.caps.contains(&v) as usize
        }
        NExpr::Match(s, _, specs) => {
            count_uses(v, s) + specs.iter().filter(|sp| sp.caps.contains(&v)).count()
        }
        NExpr::Proj(e2, _) => count_uses(v, e2),
        NExpr::Prim(_, args) => args.iter().map(|a| count_uses(v, a)).sum(),
    }
}

/// Bind `v` to the value flowing through `port` for `k` statically counted
/// uses: 0 uses erases the value, 1 use passes the port straight through,
/// k>1 uses insert a DUP chain of k-1 cells fanning out to k fresh wires.
fn bind(net: &mut Net, env: &mut Env, v: u32, port: Port, k: usize) {
    match k {
        0 => link(net, era(), port),
        1 => env.give(v, vec![port]),
        _ => {
            let outs: Vec<Port> = (0..k).map(|_| wire(net)).collect();
            let mut next = outs[k - 1];
            for j in (0..k - 1).rev() {
                let d = net.alloc(outs[j], next);
                next = dup_port(d);
            }
            link(net, next, port);
            env.give(v, outs);
        }
    }
}

/// Splice entry `fid`'s body into the net, binding `args` to its params
/// and linking the body's result to `ret`. Called by the REF-unfold rule.
pub(crate) fn instantiate(net: &mut Net, prog: &NetProg, fid: usize, args: Vec<Port>, ret: Port) {
    let entry = &prog.entries[fid];
    assert_eq!(
        args.len(),
        entry.params.len(),
        "ICE: entry {} expects {} arg(s), Ref carried {}",
        fid,
        entry.params.len(),
        args.len()
    );
    let mut env = Env::default();
    for (p, a) in entry.params.iter().zip(args) {
        let k = count_uses(*p, &entry.body);
        bind(net, &mut env, *p, a, k);
    }
    let out = inst(net, prog, &mut env, &entry.body);
    link(net, out, ret);
}

/// Build a deferred closure: capture the spec's free vars from `env` into
/// an argument chain and wrap them in a (not yet fired) `Ref` port.
fn closure(net: &mut Net, env: &mut Env, spec: &ClosureSpec) -> Port {
    let caps: Vec<Port> = spec.caps.iter().map(|v| env.take(*v)).collect();
    let head = list_alloc(net, &caps);
    ref_port(head, spec.entry)
}

/// Instantiate one expression, returning the port its value flows out of.
fn inst(net: &mut Net, prog: &NetProg, env: &mut Env, e: &NExpr) -> Port {
    match e {
        NExpr::Num(n) => Port::num(*n),
        NExpr::Flo(f) => crate::rules::flo_alloc(net, *f),
        NExpr::Var(v) => env.take(*v),
        NExpr::Op2(code, a, b) => {
            let p1 = inst(net, prog, env, a);
            let p2 = inst(net, prog, env, b);
            let w = wire(net);
            let c = net.alloc(p2, w);
            link(net, op_port(c, *code), p1);
            w
        }
        NExpr::Let(v, r, b) => {
            let pr = inst(net, prog, env, r);
            bind(net, env, *v, pr, count_uses(*v, b));
            inst(net, prog, env, b)
        }
        NExpr::Call(f, args) => {
            let ps: Vec<Port> = args.iter().map(|a| inst(net, prog, env, a)).collect();
            let head = list_alloc(net, &ps);
            let w = wire(net);
            // A Ref must meet the unfold rule, not be parked in a wire.
            net.redexes.push((ref_port(head, *f), w));
            w
        }
        NExpr::Ctor(ctag, args) => {
            let ps: Vec<Port> = args.iter().map(|a| inst(net, prog, env, a)).collect();
            con_alloc(net, *ctag, &ps)
        }
        NExpr::Tuple(items) => {
            let ps: Vec<Port> = items.iter().map(|a| inst(net, prog, env, a)).collect();
            con_alloc(net, CTAG_TUPLE, &ps)
        }
        NExpr::If(c, t, e2) => {
            let pc = inst(net, prog, env, c);
            let rt = closure(net, env, t);
            let re = closure(net, env, e2);
            let s2 = net.alloc(rt, re);
            let w = wire(net);
            let s = net.alloc(w, Port::new(Tag::Ext, s2 as u64));
            link(net, Port::new(Tag::Swi, s as u64), pc);
            w
        }
        NExpr::Match(s, mid, specs) => {
            let ps = inst(net, prog, env, s);
            let refs: Vec<Port> = specs.iter().map(|sp| closure(net, env, sp)).collect();
            let head = list_alloc(net, &refs);
            let w = wire(net);
            let m = net.alloc(w, head);
            link(net, mat_port(m, *mid), ps);
            w
        }
        NExpr::Proj(e2, mid) => {
            let pe = inst(net, prog, env, e2);
            let w = wire(net);
            let m = net.alloc(w, EMPTY);
            link(net, mat_port(m, *mid), pe);
            w
        }
        NExpr::Prim(code, args) => {
            let ps: Vec<Port> = args.iter().map(|a| inst(net, prog, env, a)).collect();
            let binop = |net: &mut Net, code: u16, p1: Port, p2: Port| -> Port {
                let w = wire(net);
                let c = net.alloc(p2, w);
                link(net, op_port(c, code), p1);
                w
            };
            match ps.len() {
                1 => binop(net, *code, ps[0], Port::num(0)),
                2 => binop(net, *code, ps[0], ps[1]),
                3 => {
                    let pair = binop(net, crate::ARR_PAIR, ps[1], ps[2]);
                    binop(net, *code, ps[0], pair)
                }
                n => panic!("ICE: builtin with {} arguments", n),
            }
        }
    }
}

/// A specialization net for real function `fid`: its parameters as fresh
/// unfilled wires (returned in order), its body instantiated against the
/// root wire (cell 0).
pub(crate) fn build_fn(net: &mut Net, prog: &NetProg, fid: usize) -> Vec<Port> {
    let root = net.alloc(EMPTY, EMPTY);
    debug_assert_eq!(root, 0, "root wire must be cell 0");
    let arity = prog.entries[fid].params.len();
    let args: Vec<Port> = (0..arity).map(|_| wire(net)).collect();
    instantiate(net, prog, fid, args.clone(), root_port());
    args
}
