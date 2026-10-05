//! The derived program and its instantiation, shared by both times: a
//! `CoreModule` lowers (in `mithril-net`) to entries whose bodies are
//! `NExpr` trees; `instantiate` splices an entry's body into any `Cells`
//! (the compile-time net or a runtime worker's arena) during `Ref`
//! unfolding — this is also where DUP chains are inserted for every
//! variable used more than once.

use crate::agents::*;
use crate::port::{Port, Tag};
use crate::rules::link;
use std::collections::{HashMap, VecDeque};

/// Opcode of the pairing pseudo-op (a ternary builtin rides a binary Op
/// whose second operand is a pair of its last two arguments).
pub const ARR_PAIR: u16 = 48;
/// Opcodes from here up are the builtins (`Core::Prim`).
pub const PRIM_BASE: u16 = 32;

/// A deferred branch/arm: `entry` to call, `caps` = free vars captured at
/// the construction site (sorted; appended after any pattern binders in
/// the entry's params).
pub struct ClosureSpec {
    pub entry: u16,
    pub caps: Vec<u32>,
}

pub enum NExpr {
    Num(i64),
    Flo(f64),
    Var(u32),
    /// opcode (BinOp/CmpOp space), strict in both operands.
    Op2(u16, Box<NExpr>, Box<NExpr>),
    Let(u32, Box<NExpr>, Box<NExpr>),
    /// Saturated call to an entry (strict in args, unfolds fuel-gated).
    Call(u16, Vec<NExpr>),
    /// 12-bit ctor tag.
    Ctor(u16, Vec<NExpr>),
    Tuple(Vec<NExpr>),
    If(Box<NExpr>, ClosureSpec, ClosureSpec),
    Match(Box<NExpr>, u16, Vec<ClosureSpec>),
    Proj(Box<NExpr>, u16),
    /// A builtin (opcode >= PRIM_BASE), strict in its arguments.
    Prim(u16, Vec<NExpr>),
    /// A closure: its body is instantiated eagerly as a net region whose
    /// free variables are the enclosing wires (capture is wiring).
    Lam(u32, Box<NExpr>),
    App(Box<NExpr>, Box<NExpr>),
}

pub enum MatchMeta {
    /// Ctor tag per arm, in arm order.
    Arms(Vec<u16>),
    /// Tuple projection index.
    Proj(usize),
}

pub struct Entry {
    pub params: Vec<u32>,
    pub body: NExpr,
}

/// Per-instantiation variable environment: each bound variable owns a
/// queue of use-ports (length = its statically counted use count), popped
/// front-first as occurrences are reached left-to-right.
#[derive(Default)]
pub struct Env(HashMap<u32, VecDeque<Port>>);

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
pub fn count_uses(v: u32, e: &NExpr) -> usize {
    match e {
        NExpr::Num(_) | NExpr::Flo(_) => 0,
        NExpr::Var(u) => (*u == v) as usize,
        NExpr::Op2(_, a, b) | NExpr::App(a, b) => count_uses(v, a) + count_uses(v, b),
        NExpr::Let(u, r, b) => count_uses(v, r) + if *u == v { 0 } else { count_uses(v, b) },
        NExpr::Call(_, args) | NExpr::Ctor(_, args) | NExpr::Tuple(args) | NExpr::Prim(_, args) => {
            args.iter().map(|a| count_uses(v, a)).sum()
        }
        NExpr::If(c, t, e2) => {
            count_uses(v, c) + t.caps.contains(&v) as usize + e2.caps.contains(&v) as usize
        }
        NExpr::Match(s, _, specs) => {
            count_uses(v, s) + specs.iter().filter(|sp| sp.caps.contains(&v)).count()
        }
        NExpr::Proj(e2, _) => count_uses(v, e2),
        NExpr::Lam(u, b) => if *u == v { 0 } else { count_uses(v, b) },
    }
}

/// Bind `v` to the value flowing through `port` for `k` statically counted
/// uses: 0 uses erases the value, 1 use passes the port straight through,
/// k>1 uses insert a DUP chain of k-1 cells fanning out to k fresh wires.
fn bind<C: Cells>(net: &mut C, env: &mut Env, v: u32, port: Port, k: usize) {
    match k {
        0 => link(net, era(), port),
        1 => env.give(v, vec![port]),
        _ => {
            let outs: Vec<Port> = (0..k).map(|_| wire(net)).collect();
            let mut next = outs[k - 1];
            for j in (0..k - 1).rev() {
                // one label per cell: every copy it makes is a binary
                // superposition of its own
                let label = net.fresh_label();
                let d = net.alloc(outs[j], next);
                next = dup_port(d, label);
            }
            link(net, next, port);
            env.give(v, outs);
        }
    }
}

/// Splice entry `fid`'s body into the net, binding `args` to its params
/// and linking the body's result to `ret`. Called by the REF-unfold rule.
pub fn instantiate<C: Cells>(net: &mut C, entries: &[Entry], fid: usize, args: Vec<Port>, ret: Port) {
    let entry = &entries[fid];
    assert_eq!(
        args.len(),
        entry.params.len(),
        "ICE: entry {} expects {} arg(s), Ref carried {} ({:?})",
        fid,
        entry.params.len(),
        args.len(),
        args.iter().map(|a| (a.tag(), a.payload())).collect::<Vec<_>>()
    );
    let mut env = Env::default();
    for (p, a) in entry.params.iter().zip(args) {
        let k = count_uses(*p, &entry.body);
        bind(net, &mut env, *p, a, k);
    }
    let out = inst(net, entries, &mut env, &entry.body);
    link(net, out, ret);
}

/// Build a deferred closure: capture the spec's free vars from `env` into
/// an argument chain and wrap them in a (not yet fired) `Ref` port.
fn closure<C: Cells>(net: &mut C, env: &mut Env, spec: &ClosureSpec) -> Port {
    let caps: Vec<Port> = spec.caps.iter().map(|v| env.take(*v)).collect();
    let head = list_alloc(net, &caps);
    ref_port(head, spec.entry)
}

/// A binary agent `mk(cell)` with cell `[second, ret]`, linked to `first`;
/// returns ret.
fn agent2<C: Cells>(net: &mut C, mk: impl Fn(u32) -> Port, first: Port, second: Port) -> Port {
    let w = wire(net);
    let c = net.alloc(second, w);
    link(net, mk(c), first);
    w
}

/// A selector `mk(cell)` with cell `[ret, rest]`, linked to its input;
/// returns ret.
fn selector<C: Cells>(net: &mut C, mk: impl Fn(u32) -> Port, input: Port, rest: Port) -> Port {
    let w = wire(net);
    let m = net.alloc(w, rest);
    link(net, mk(m), input);
    w
}

fn insts<C: Cells>(net: &mut C, prog: &[Entry], env: &mut Env, xs: &[NExpr]) -> Vec<Port> {
    xs.iter().map(|a| inst(net, prog, env, a)).collect()
}

/// Instantiate one expression, returning the port its value flows out of.
/// The erasure of an expression that is never built: the uses of outer variables it
/// would have taken are linked to ERA, exactly as if ERA had met its value first.
fn erase<C: Cells>(net: &mut C, env: &mut Env, e: &NExpr, bound: &mut Vec<u32>) {
    let (uses, kids): (Vec<u32>, Vec<&NExpr>) = match e {
        NExpr::Var(v) => (vec![*v], vec![]),
        NExpr::If(c, t, f) => ([&t.caps[..], &f.caps[..]].concat(), vec![c]),
        NExpr::Match(s, _, specs) => (specs.iter().flat_map(|sp| sp.caps.clone()).collect(), vec![s]),
        NExpr::Op2(_, a, b) | NExpr::App(a, b) => (vec![], vec![a, b]),
        NExpr::Call(_, xs) | NExpr::Ctor(_, xs) | NExpr::Tuple(xs) | NExpr::Prim(_, xs) => {
            (vec![], xs.iter().collect())
        }
        NExpr::Proj(a, _) => (vec![], vec![a]),
        NExpr::Let(u, r, b) => {
            erase(net, env, r, bound);
            return erase_under(net, env, *u, b, bound);
        }
        NExpr::Lam(u, b) => return erase_under(net, env, *u, b, bound),
        NExpr::Num(_) | NExpr::Flo(_) => return,
    };
    for v in uses.into_iter().filter(|v| !bound.contains(v)) {
        let p = env.take(v);
        link(net, era(), p);
    }
    kids.into_iter().for_each(|k| erase(net, env, k, bound));
}

/// `erase` of `b` with `u` bound inside it (its uses are not outer ones).
fn erase_under<C: Cells>(net: &mut C, env: &mut Env, u: u32, b: &NExpr, bound: &mut Vec<u32>) {
    bound.push(u);
    erase(net, env, b, bound);
    bound.pop();
}

fn inst<C: Cells>(net: &mut C, prog: &[Entry], env: &mut Env, e: &NExpr) -> Port {
    match e {
        NExpr::Num(n) => Port::int(*n),
        NExpr::Flo(f) => net.alloc_flo(*f),
        NExpr::Var(v) => env.take(*v),
        NExpr::Op2(code, a, b) => {
            let p1 = inst(net, prog, env, a);
            let p2 = inst(net, prog, env, b);
            agent2(net, |c| op_port(c, *code), p1, p2)
        }
        NExpr::Let(v, r, b) => {
            match count_uses(*v, b) {
                // a dead binding's value is never built
                0 => erase(net, env, r, &mut vec![]),
                k => {
                    let pr = inst(net, prog, env, r);
                    bind(net, env, *v, pr, k)
                }
            }
            inst(net, prog, env, b)
        }
        NExpr::Call(f, args) => {
            let ps = insts(net, prog, env, args);
            let head = list_alloc(net, &ps);
            let w = wire(net);
            // A Ref must meet the unfold rule, not be parked in a wire.
            net.push_redex(ref_port(head, *f), w);
            w
        }
        NExpr::Ctor(ctag, args) => {
            let ps = insts(net, prog, env, args);
            con_alloc(net, *ctag, &ps)
        }
        NExpr::Tuple(items) => {
            let ps = insts(net, prog, env, items);
            con_alloc(net, CTAG_TUPLE, &ps)
        }
        NExpr::If(c, t, e2) => {
            let pc = inst(net, prog, env, c);
            let rt = closure(net, env, t);
            let re = closure(net, env, e2);
            let s2 = net.alloc(rt, re);
            selector(net, |s| Port::new(Tag::Swi, s as u64), pc, Port::new(Tag::Ext, s2 as u64))
        }
        NExpr::Match(s, mid, specs) => {
            let ps = inst(net, prog, env, s);
            let refs: Vec<Port> = specs.iter().map(|sp| closure(net, env, sp)).collect();
            let head = list_alloc(net, &refs);
            selector(net, |m| mat_port(m, *mid), ps, head)
        }
        NExpr::Proj(e2, mid) => {
            let pe = inst(net, prog, env, e2);
            selector(net, |m| mat_port(m, *mid), pe, EMPTY)
        }
        NExpr::Prim(code, args) => {
            let ps = insts(net, prog, env, args);
            let binop = |net: &mut C, code: u16, p1: Port, p2: Port| agent2(net, |c| op_port(c, code), p1, p2);
            match ps.len() {
                1 => binop(net, *code, ps[0], Port::num(0)),
                2 => binop(net, *code, ps[0], ps[1]),
                3 => {
                    let pair = binop(net, ARR_PAIR, ps[1], ps[2]);
                    binop(net, *code, ps[0], pair)
                }
                n => panic!("ICE: builtin with {} arguments", n),
            }
        }
        NExpr::Lam(x, body) => {
            // Lam cell [param, body]: the parameter is a wire the body reads
            // (dup-fanned for several uses), the body's value flows to the
            // body wire; beta links the argument to the one and the return
            // to the other
            let p = wire(net);
            let b = wire(net);
            bind(net, env, *x, p, count_uses(*x, body));
            let out = inst(net, prog, env, body);
            link(net, out, b);
            let l = net.alloc(p, b);
            Port::new(Tag::Lam, l as u64)
        }
        NExpr::App(f, a) => {
            let pf = inst(net, prog, env, f);
            let pa = inst(net, prog, env, a);
            agent2(net, |c| Port::new(Tag::App, c as u64), pf, pa)
        }
    }
}

