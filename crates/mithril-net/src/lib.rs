//! mithril-net: interaction-net construction from Core IR, the interaction
//! rule table, the compile-time (fuel-bounded) reducer, and value readback.
//!
//! # Encoding
//!
//! The net is `mithril_core::net::Net`: an arena of 2-port cells plus a
//! worklist of port pairs (redexes). Per-tag cell/payload layouts:
//!
//! - `Var(w)`   — one end of a *wire*: cell `w` starts `[EMPTY, EMPTY]`;
//!   the first side to arrive stores its port in slot 0, the second side
//!   takes it (freeing the cell) and the two meet. A wire has exactly two
//!   ends, so take-and-free is safe.
//! - `Num`      — unboxed i56 immediate (no cell).
//! - `Flo(a)`   — boxed f64: cell `a` holds the bit pattern split as two
//!   `Num`-tagged ports (low 56 bits, high 8 bits) so `Net::dump` stays
//!   printable.
//! - `Con(a,k,n)` — constructor, payload `addr:40|ctor:12|arity:4`; `n` is
//!   the *remaining* field count of the chain: `n<=2` fields sit in cell
//!   `a`, `n>2` puts field 0 in slot 0 and a `Con(_,k,n-1)` continuation in
//!   slot 1. Tags `0xFFF`/`0xFFE` are reserved for tuples / desugar's
//!   unreachable-match sentinel.
//! - `Op(a,op)` — payload `addr:40|opcode:16`; cell = `[other operand,
//!   ret]`. Opcodes: BinOp 0..=10, CmpOp 16..=21, bit 8 = "operands
//!   swapped" (the first-arrived operand was stored into slot 0).
//! - `Swi(a)`   — cell `a` = `[ret, Ext(b)]`, cell `b` = `[then closure,
//!   else closure]`; branches are deferred `Ref` closures so untaken code
//!   is never built (this is how If stays lazy and recursion terminates).
//! - `Mat(a,m)` — payload `addr:40|match_id:16`; cell `a` = `[ret, arm
//!   list]` where the arm list is a `Ref`-closure chain in arm order and
//!   `match_id` indexes the module-derived match metadata (arm ctor tags,
//!   or a tuple projection index — `Proj` is compiled as a 1-way MAT).
//! - `Ref(h,f)` — payload `head+1:40|entry:16`; a (possibly partial)
//!   saturated call: `h` heads a chain of argument list cells, `f` indexes
//!   the derived entry table (real fns first, then lifted If branches and
//!   Match arms). `head+1 == 0` means "no args".
//! - `Dup(a,l)` — payload `addr:32|label:24`; cell = the two copy targets.
//!   Every sharing site gets a fresh label; a copy of a dup (through a
//!   constructor, a lambda, a commutation) carries the copier's label, so
//!   two dups that meet face to face annihilate exactly when they are the
//!   two halves of one copy and commute otherwise.
//! - `Era`, `Lam(a)`, `App(a)` — standard; `Lam`/`App` cells are
//!   `[param, body]` / `[arg, ret]`. LAM/APP are unreachable from Core v1
//!   (desugar rejects lambdas) but APP-LAM beta is implemented.
//! - `Ext`      — internal chain pointers (`Ext(addr)`) and the `EMPTY`
//!   sentinel (all-ones payload); never a redex side.
//!
//! List cells (`Ref` args, MAT arms) are `[item, next]` with `next` either
//! `Ext(addr)` or `EMPTY` — self-terminating, so erasure never needs
//! arity metadata.
//!
//! # Derived program
//!
//! `NetProg::new(m)` deterministically lowers the `CoreModule`: every
//! `Core::If` branch and `Core::Match` arm is lambda-lifted into a fresh
//! entry whose params are `binders ++ sorted free vars`; `Ref` cells point
//! at entries. Both `build` and `reduce` derive the same table from the
//! same module, so it never needs to live inside the `Net`.

pub mod build;
pub mod reduce;
pub mod residual;
pub mod rules;

use mithril_core::net::Net;
use mithril_core::port::{Port, Tag};
use mithril_front::ast::{BinOp, CmpOp};
use mithril_front::core::{Core, CoreModule, UNREACHABLE_CTOR};
use mithril_front::Diag;
use std::collections::{BTreeMap, BTreeSet};

pub use build::{build, root_port};
pub use reduce::{readback, reduce, specialize, SpecReport};
pub use rules::link;

/// Sentinel port marking (a) an unfilled wire-cell slot and (b) the end of
/// an argument/arm list chain. Encoded as `Ext` with an all-ones payload so
/// it cannot collide with a real `Ext(addr)` chain pointer (addresses are
/// 40-bit) and stays printable by `Net::dump`.
pub const EMPTY: Port = Port(((Tag::Ext as u64) << 56) | ((1u64 << 56) - 1));

/// 12-bit constructor tag reserved for tuples.
pub const CTAG_TUPLE: u16 = 0xFFF;
/// 12-bit constructor tag for desugar's unreachable-match sentinel.
pub const CTAG_UNREACHABLE: u16 = 0xFFE;

/// Erasure port.
pub(crate) fn era() -> Port {
    Port::new(Tag::Era, 0)
}

/// Allocate a fresh wire cell and return one of its (interchangeable) ends.
pub fn wire(net: &mut Net) -> Port {
    let w = net.alloc(EMPTY, EMPTY);
    Port::new(Tag::Var, w as u64)
}

// ---- opcode space (Op payload low 16 bits) ----

/// "Operands swapped" flag: slot 0 holds the *first* operand.
pub(crate) const OP_FLIP: u16 = 1 << 8;

/// Opcodes from here up are the builtins (`Core::Prim`), strict agents like
/// the arithmetic ops: binary32 ops fold on numbers; array ops are opaque at
/// compile time (arrays are runtime heap values) and always stay residual.
/// A unary builtin rides a binary Op with a `Num(0)` second operand; the
/// ternary `array_set(a, i, v)` is `ARRSET(a, ARR_PAIR(i, v))`.
pub(crate) const PRIM_BASE: u16 = 32;
pub(crate) const ARR_PAIR: u16 = 44;

pub(crate) fn prim_code(p: mithril_front::core::Prim) -> u16 {
    use mithril_front::core::Prim::*;
    match p {
        F32Add => 32,
        F32Sub => 33,
        F32Mul => 34,
        F32Div => 35,
        F32Sqrt => 36,
        F32Lt => 37,
        F32FromU32 => 38,
        F32ToU32 => 39,
        ArrNew => 40,
        ArrGet => 41,
        ArrLen => 42,
        ArrSet => 43,
    }
}

/// The builtin of an opcode and whether it is unary.
pub(crate) fn prim_of_code(code: u16) -> (mithril_front::core::Prim, bool) {
    use mithril_front::core::Prim::*;
    match code {
        32 => (F32Add, false),
        33 => (F32Sub, false),
        34 => (F32Mul, false),
        35 => (F32Div, false),
        36 => (F32Sqrt, true),
        37 => (F32Lt, false),
        38 => (F32FromU32, true),
        39 => (F32ToU32, true),
        40 => (ArrNew, false),
        41 => (ArrGet, false),
        42 => (ArrLen, true),
        43 => (ArrSet, false),
        c => panic!("ICE: opcode {} is not a builtin", c),
    }
}

pub fn opcode_bin(op: BinOp) -> u16 {
    match op {
        BinOp::Add => 0,
        BinOp::Sub => 1,
        BinOp::Mul => 2,
        BinOp::Div => 3,
        BinOp::FloorDiv => 4,
        BinOp::Mod => 5,
        BinOp::Shl => 6,
        BinOp::Shr => 7,
        BinOp::BitAnd => 8,
        BinOp::BitOr => 9,
        BinOp::BitXor => 10,
    }
}

pub(crate) fn opcode_cmp(op: CmpOp) -> u16 {
    match op {
        CmpOp::Lt => 16,
        CmpOp::Le => 17,
        CmpOp::Gt => 18,
        CmpOp::Ge => 19,
        CmpOp::Eq => 20,
        CmpOp::Ne => 21,
    }
}

// ---- payload packing helpers ----

pub fn op_port(addr: u32, code: u16) -> Port {
    Port::new(Tag::Op, ((addr as u64) << 16) | code as u64)
}
pub(crate) fn op_addr(p: Port) -> u32 {
    (p.payload() >> 16) as u32
}
pub(crate) fn op_code(p: Port) -> u16 {
    (p.payload() & 0xFFFF) as u16
}

/// `Dup(addr, label)` — payload `addr:32|label:24`.
pub fn dup_port(addr: u32, label: u32) -> Port {
    Port::new(Tag::Dup, ((addr as u64) << 24) | (label & 0xFF_FFFF) as u64)
}
pub(crate) fn dup_addr(p: Port) -> u32 {
    (p.payload() >> 24) as u32
}
pub(crate) fn dup_label(p: Port) -> u32 {
    (p.payload() & 0xFF_FFFF) as u32
}
/// A fresh label for a new sharing site.
pub fn fresh_label(net: &mut Net) -> u32 {
    let l = net.labels;
    net.labels = net.labels.wrapping_add(1) & 0xFF_FFFF;
    if net.labels == 0 {
        net.labels = 1;
    }
    l
}

pub(crate) fn mat_port(addr: u32, match_id: u16) -> Port {
    Port::new(Tag::Mat, ((addr as u64) << 16) | match_id as u64)
}
pub(crate) fn mat_addr(p: Port) -> u32 {
    (p.payload() >> 16) as u32
}
pub(crate) fn mat_id(p: Port) -> u16 {
    (p.payload() & 0xFFFF) as u16
}

/// `head` is a list head (`Ext(addr)` or `EMPTY`); stored biased by one so
/// that 0 means "no args".
pub fn ref_port(head: Port, entry: u16) -> Port {
    let h = if head == EMPTY { 0 } else { head.payload() + 1 };
    Port::new(Tag::Ref, (h << 16) | entry as u64)
}
pub(crate) fn ref_head(p: Port) -> Port {
    let h = p.payload() >> 16;
    if h == 0 {
        EMPTY
    } else {
        Port::new(Tag::Ext, h - 1)
    }
}
pub(crate) fn ref_entry(p: Port) -> u16 {
    (p.payload() & 0xFFFF) as u16
}

// ---- list chains ([item, Ext(next)|EMPTY] cells) ----

pub fn list_alloc(net: &mut Net, items: &[Port]) -> Port {
    let mut head = EMPTY;
    for &it in items.iter().rev() {
        let a = net.alloc(it, head);
        head = Port::new(Tag::Ext, a as u64);
    }
    head
}

/// The items of a list chain, without freeing it.
pub(crate) fn list_items(net: &Net, mut head: Port) -> Vec<Port> {
    let mut out = Vec::new();
    while head != EMPTY {
        debug_assert_eq!(head.tag(), Tag::Ext);
        let c = net.cell(head.payload() as u32);
        out.push(Port(c[0]));
        head = Port(c[1]);
    }
    out
}

/// Walk and free a list chain, returning the items in order.
pub(crate) fn list_collect(net: &mut Net, mut head: Port) -> Vec<Port> {
    let mut out = Vec::new();
    while head != EMPTY {
        debug_assert_eq!(head.tag(), Tag::Ext);
        let a = head.payload() as u32;
        let c = net.cell(a);
        net.free_cell(a);
        out.push(Port(c[0]));
        head = Port(c[1]);
    }
    out
}

// ---- constructor chains ----

/// The 4-bit arity field is the *remaining* field count of a chain,
/// saturating at 15 (as in the runtime's `mk_con`): a cell with count 15
/// holds one field and a continuation; the count becomes exact once it
/// drops below 15, so a chain of any length reads back by following
/// continuations while the count exceeds 2.
pub(crate) fn con_alloc(net: &mut Net, ctag: u16, fields: &[Port]) -> Port {
    let n = fields.len();
    match n {
        0 => Port::con(0, ctag, 0),
        1 => {
            let a = net.alloc(fields[0], EMPTY);
            Port::con(a as u64, ctag, 1)
        }
        2 => {
            let a = net.alloc(fields[0], fields[1]);
            Port::con(a as u64, ctag, 2)
        }
        _ => {
            let rest = con_alloc(net, ctag, &fields[1..]);
            let a = net.alloc(fields[0], rest);
            Port::con(a as u64, ctag, n.min(15) as u8)
        }
    }
}

/// Walk and free a constructor chain, returning the field ports in order.
pub(crate) fn con_collect(net: &mut Net, mut p: Port) -> Vec<Port> {
    let mut out = Vec::new();
    loop {
        let n = p.con_arity();
        let a = p.con_addr() as u32;
        match n {
            0 => break,
            1 => {
                let c = net.cell(a);
                net.free_cell(a);
                out.push(Port(c[0]));
                break;
            }
            2 => {
                let c = net.cell(a);
                net.free_cell(a);
                out.push(Port(c[0]));
                out.push(Port(c[1]));
                break;
            }
            _ => {
                let c = net.cell(a);
                net.free_cell(a);
                out.push(Port(c[0]));
                p = Port(c[1]);
                debug_assert_eq!(p.tag(), Tag::Con);
            }
        }
    }
    out
}

// ---- derived program: lambda-lifted Core ----

/// A deferred branch/arm: `entry` to call, `caps` = free vars captured at
/// the construction site (sorted; appended after any pattern binders in
/// the entry's params).
pub(crate) struct ClosureSpec {
    pub entry: u16,
    pub caps: Vec<u32>,
}

pub(crate) enum NExpr {
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

pub(crate) enum MatchMeta {
    /// Ctor tag per arm, in arm order.
    Arms(Vec<u16>),
    /// Tuple projection index.
    Proj(usize),
}

pub(crate) struct Entry {
    pub params: Vec<u32>,
    pub body: NExpr,
}

/// What the REF-unfold rule does with a call to a real function.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Mode {
    /// evaluation: unfold every call
    Eval,
    /// specialization: unfold a call only where `NetProg::inline` allows;
    /// other calls stay residual
    Specialize,
}

pub(crate) struct NetProg {
    pub entries: Vec<Entry>,
    pub metas: Vec<MatchMeta>,
    /// real functions are entries 0..nfns; lifted branches/arms follow
    pub nfns: usize,
    pub mode: Mode,
    /// per real function: unfold its calls during specialization (it is
    /// on no call cycle and small, i.e. inlining it cannot recurse)
    pub inline: Vec<bool>,
    /// per real function: the real functions it can (transitively) call
    pub reach: Vec<BTreeSet<u32>>,
    /// per entry (lifted ones included): the real functions its body calls
    /// directly, closures' bodies included
    pub direct: Vec<BTreeSet<u32>>,
}

impl NetProg {
    /// Deterministically derived from the module alone, so `build` and
    /// `reduce` agree on entry ids without storing the table in the `Net`.
    pub(crate) fn new(m: &CoreModule) -> NetProg {
        let nfns = m.fns.len();
        let mut prog = NetProg { entries: Vec::new(), metas: Vec::new(), nfns, mode: Mode::Eval, inline: vec![false; nfns], reach: Vec::new(), direct: Vec::new() };
        for f in &m.fns {
            prog.entries.push(Entry { params: (0..f.arity as u32).collect(), body: NExpr::Num(0) });
        }
        for (i, f) in m.fns.iter().enumerate() {
            let body = lower(&f.body, &mut prog);
            prog.entries[i].body = body;
        }
        prog.inline = inline_policy(m);
        prog.reach = reach_relation(m);
        prog.direct = prog.entries.iter().map(|e| { let mut s = BTreeSet::new(); nexpr_calls(&e.body, &prog, &mut s); s }).collect();
        prog
    }
}

/// Real functions reachable from each real function through calls.
fn reach_relation(m: &CoreModule) -> Vec<BTreeSet<u32>> {
    let n = m.fns.len();
    let callees: Vec<BTreeSet<u32>> = m.fns.iter().map(|f| { let mut s = BTreeSet::new(); calls_of(&f.body, &mut s); s }).collect();
    (0..n)
        .map(|f| {
            let mut seen = BTreeSet::new();
            let mut stack: Vec<u32> = callees[f].iter().copied().collect();
            while let Some(g) = stack.pop() {
                if seen.insert(g) {
                    stack.extend(callees[g as usize].iter().copied());
                }
            }
            seen
        })
        .collect()
}

/// Real functions called directly by a lowered body, looking through the
/// closures of its branches and arms.
fn nexpr_calls(e: &NExpr, prog: &NetProg, out: &mut BTreeSet<u32>) {
    match e {
        NExpr::Num(_) | NExpr::Flo(_) | NExpr::Var(_) => {}
        NExpr::Op2(_, a, b) | NExpr::Let(_, a, b) => {
            nexpr_calls(a, prog, out);
            nexpr_calls(b, prog, out);
        }
        NExpr::Call(f, args) => {
            out.insert(*f as u32);
            args.iter().for_each(|a| nexpr_calls(a, prog, out));
        }
        NExpr::Ctor(_, args) | NExpr::Tuple(args) | NExpr::Prim(_, args) => args.iter().for_each(|a| nexpr_calls(a, prog, out)),
        NExpr::If(c, t, e2) => {
            nexpr_calls(c, prog, out);
            nexpr_calls(&prog.entries[t.entry as usize].body, prog, out);
            nexpr_calls(&prog.entries[e2.entry as usize].body, prog, out);
        }
        NExpr::Match(s, _, specs) => {
            nexpr_calls(s, prog, out);
            specs.iter().for_each(|sp| nexpr_calls(&prog.entries[sp.entry as usize].body, prog, out));
        }
        NExpr::Proj(a, _) => nexpr_calls(a, prog, out),
        NExpr::Lam(_, b) => nexpr_calls(b, prog, out),
        NExpr::App(f, a) => {
            nexpr_calls(f, prog, out);
            nexpr_calls(a, prog, out);
        }
    }
}

/// Inlining budget for acyclic callees during specialization (Core nodes).
const INLINE_MAX: usize = 96;

/// Which real functions a specialization unfolds at their call sites:
/// call-free functions whose body is small (the leaf rule this replaces;
/// a call-free body cannot recurse). Larger or calling functions stay
/// calls unless their arguments are all known (then they are evaluated).
fn inline_policy(m: &CoreModule) -> Vec<bool> {
    let n = m.fns.len();
    let callees: Vec<BTreeSet<u32>> = m.fns.iter().map(|f| { let mut s = BTreeSet::new(); calls_of(&f.body, &mut s); s }).collect();
    // cyclic: reaches itself
    let mut cyclic = vec![false; n];
    for f in 0..n {
        let mut seen = vec![false; n];
        let mut stack: Vec<u32> = callees[f].iter().copied().collect();
        while let Some(g) = stack.pop() {
            if g as usize == f { cyclic[f] = true; break; }
            if std::mem::replace(&mut seen[g as usize], true) { continue; }
            stack.extend(callees[g as usize].iter().copied());
        }
    }
    let mut sites = vec![0usize; n];
    for f in 0..n {
        for g in &callees[f] {
            sites[*g as usize] += count_calls(&m.fns[f].body, *g);
        }
    }
    let _ = sites;
    (0..n).map(|f| f as u32 != m.main && !cyclic[f] && callees[f].is_empty() && core_size(&m.fns[f].body) <= INLINE_MAX).collect()
}

fn calls_of(c: &Core, out: &mut BTreeSet<u32>) {
    match c {
        Core::Call(f, args) => { out.insert(*f); args.iter().for_each(|a| calls_of(a, out)); }
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => {}
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) | Core::Let(_, a, b) => { calls_of(a, out); calls_of(b, out); }
        Core::If(a, b, c2) => { calls_of(a, out); calls_of(b, out); calls_of(c2, out); }
        Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().for_each(|x| calls_of(x, out)),
        Core::Match(s, arms) => { calls_of(s, out); arms.iter().for_each(|(_, _, b)| calls_of(b, out)); }
        Core::Proj(a, _) | Core::Lam(_, a) => calls_of(a, out),
        Core::App(f, a) => { calls_of(f, out); calls_of(a, out); }
    }
}

fn count_calls(c: &Core, g: u32) -> usize {
    match c {
        Core::Call(f, args) => (*f == g) as usize + args.iter().map(|a| count_calls(a, g)).sum::<usize>(),
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => 0,
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) | Core::Let(_, a, b) => count_calls(a, g) + count_calls(b, g),
        Core::If(a, b, c2) => count_calls(a, g) + count_calls(b, g) + count_calls(c2, g),
        Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().map(|x| count_calls(x, g)).sum(),
        Core::Match(s, arms) => count_calls(s, g) + arms.iter().map(|(_, _, b)| count_calls(b, g)).sum::<usize>(),
        Core::Proj(a, _) | Core::Lam(_, a) => count_calls(a, g),
        Core::App(f, a) => count_calls(f, g) + count_calls(a, g),
    }
}

pub(crate) fn core_size(c: &Core) -> usize {
    match c {
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => 1,
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) | Core::Let(_, a, b) => 1 + core_size(a) + core_size(b),
        Core::If(a, b, c2) => 1 + core_size(a) + core_size(b) + core_size(c2),
        Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => 1 + xs.iter().map(core_size).sum::<usize>(),
        Core::Match(s, arms) => 1 + core_size(s) + arms.iter().map(|(_, _, b)| core_size(b)).sum::<usize>(),
        Core::Proj(a, _) | Core::Lam(_, a) => 1 + core_size(a),
        Core::App(f, a) => 1 + core_size(f) + core_size(a),
    }
}

fn lift(prog: &mut NetProg, params: Vec<u32>, body: &Core) -> u16 {
    let id = prog.entries.len();
    assert!(id < (1 << 16), "ICE: lifted entry table overflows 16-bit Ref ids");
    prog.entries.push(Entry { params, body: NExpr::Num(0) });
    let b = lower(body, prog);
    prog.entries[id].body = b;
    id as u16
}

fn close(prog: &mut NetProg, body: &Core, binders: &[u32]) -> ClosureSpec {
    let caps: Vec<u32> = free_vars(body).into_iter().filter(|v| !binders.contains(v)).collect();
    let mut params = binders.to_vec();
    params.extend(caps.iter().copied());
    let entry = lift(prog, params, body);
    ClosureSpec { entry, caps }
}

fn ctag_of(cid: u32) -> u16 {
    if cid == UNREACHABLE_CTOR {
        CTAG_UNREACHABLE
    } else {
        assert!(cid < CTAG_UNREACHABLE as u32, "ICE: ctor id {} exceeds the 12-bit cell tag space", cid);
        cid as u16
    }
}

fn lower(c: &Core, prog: &mut NetProg) -> NExpr {
    match c {
        Core::Num(n) => NExpr::Num(*n),
        Core::Flo(f) => NExpr::Flo(*f),
        Core::Var(v) => NExpr::Var(*v),
        Core::Op2(op, a, b) => NExpr::Op2(opcode_bin(*op), Box::new(lower(a, prog)), Box::new(lower(b, prog))),
        Core::Cmp(op, a, b) => NExpr::Op2(opcode_cmp(*op), Box::new(lower(a, prog)), Box::new(lower(b, prog))),
        Core::If(c1, t, e) => {
            let cond = lower(c1, prog);
            let st = close(prog, t, &[]);
            let se = close(prog, e, &[]);
            NExpr::If(Box::new(cond), st, se)
        }
        Core::Let(v, r, b) => NExpr::Let(*v, Box::new(lower(r, prog)), Box::new(lower(b, prog))),
        Core::Call(f, args) => NExpr::Call(*f as u16, args.iter().map(|a| lower(a, prog)).collect()),
        Core::Ctor(cid, args) | Core::Reuse(_, cid, args) => NExpr::Ctor(ctag_of(*cid), args.iter().map(|a| lower(a, prog)).collect()),
        Core::Tuple(items) => NExpr::Tuple(items.iter().map(|a| lower(a, prog)).collect()),
        Core::Match(s, arms) => {
            let scrut = lower(s, prog);
            let mid = prog.metas.len();
            assert!(mid < (1 << 16), "ICE: match table overflows 16-bit Mat ids");
            prog.metas.push(MatchMeta::Proj(0)); // placeholder, patched below
            let mut tags = Vec::new();
            let mut specs = Vec::new();
            for (cid, binders, body) in arms {
                tags.push(ctag_of(*cid));
                specs.push(close(prog, body, binders));
            }
            prog.metas[mid] = MatchMeta::Arms(tags);
            NExpr::Match(Box::new(scrut), mid as u16, specs)
        }
        Core::Proj(e, i) => {
            let mid = prog.metas.len();
            assert!(mid < (1 << 16), "ICE: match table overflows 16-bit Mat ids");
            prog.metas.push(MatchMeta::Proj(*i));
            NExpr::Proj(Box::new(lower(e, prog)), mid as u16)
        }
        Core::Prim(p, args) => NExpr::Prim(prim_code(*p), args.iter().map(|a| lower(a, prog)).collect()),
        Core::Lam(x, b) => NExpr::Lam(*x, Box::new(lower(b, prog))),
        Core::App(f, a) => NExpr::App(Box::new(lower(f, prog)), Box::new(lower(a, prog))),
    }
}

fn free_vars(c: &Core) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    fv(c, &mut out);
    out
}

fn fv(c: &Core, out: &mut BTreeSet<u32>) {
    match c {
        Core::Num(_) | Core::Flo(_) => {}
        Core::Var(v) => {
            out.insert(*v);
        }
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
            fv(a, out);
            fv(b, out);
        }
        Core::If(c1, t, e) => {
            fv(c1, out);
            fv(t, out);
            fv(e, out);
        }
        Core::Let(v, r, b) => {
            fv(r, out);
            let mut inner = BTreeSet::new();
            fv(b, &mut inner);
            inner.remove(v);
            out.extend(inner);
        }
        Core::Call(_, args) | Core::Ctor(_, args) | Core::Tuple(args) | Core::Reuse(_, _, args) | Core::Prim(_, args) => {
            for a in args {
                fv(a, out);
            }
        }
        Core::Match(s, arms) => {
            fv(s, out);
            for (_, binders, body) in arms {
                let mut inner = BTreeSet::new();
                fv(body, &mut inner);
                for b in binders {
                    inner.remove(b);
                }
                out.extend(inner);
            }
        }
        Core::Proj(e, _) => fv(e, out),
        Core::Lam(x, b) => {
            let mut inner = BTreeSet::new();
            fv(b, &mut inner);
            inner.remove(x);
            out.extend(inner);
        }
        Core::App(f, a) => {
            fv(f, out);
            fv(a, out);
        }
    }
}

// ---- clone discipline ----

/// Every variable use is bound (a param, `Let`, match binder or lambda
/// parameter), and no closure is applied to itself: `App(f, Var v)` with
/// `v` free in `f` (through `Let` aliases) is the self-duplicating clone
/// that labelled dups cannot copy correctly (the interaction-combinator
/// "oracle" cases). Everything else the labelled rules copy soundly: a
/// value shared through a constructor, a closure applied to values
/// computed from itself (`f(f(x))`), closures capturing closures.
pub fn check_clone_discipline(m: &CoreModule) -> Result<(), Diag> {
    for f in &m.fns {
        let mut bound: BTreeSet<u32> = (0..f.arity as u32).collect();
        check_bound(&f.body, &mut bound, &f.name)?;
        check_self_app(&f.body, &mut BTreeMap::new(), &f.name)?;
    }
    Ok(())
}

/// Reject `App(f, Var v)` where `v` (or a `Let` alias of it) occurs in `f`.
fn check_self_app(c: &Core, alias: &mut BTreeMap<u32, u32>, fname: &str) -> Result<(), Diag> {
    let canon = |v: u32, alias: &BTreeMap<u32, u32>| *alias.get(&v).unwrap_or(&v);
    match c {
        Core::App(f, a) => {
            check_self_app(f, alias, fname)?;
            check_self_app(a, alias, fname)?;
            if let Core::Var(v) = &**a {
                let v = canon(*v, alias);
                let fvs: BTreeSet<u32> = free_vars(f).into_iter().map(|u| canon(u, alias)).collect();
                if fvs.contains(&v) {
                    return Err(Diag::new(0, format!("clone discipline: closure v{} is applied to itself in function '{}'", v, fname)));
                }
            }
            Ok(())
        }
        Core::Let(x, r, b) => {
            check_self_app(r, alias, fname)?;
            if let Core::Var(v) = &**r {
                let v = canon(*v, alias);
                alias.insert(*x, v);
            }
            check_self_app(b, alias, fname)
        }
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => Ok(()),
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
            check_self_app(a, alias, fname)?;
            check_self_app(b, alias, fname)
        }
        Core::If(a, b, c2) => {
            check_self_app(a, alias, fname)?;
            check_self_app(b, alias, fname)?;
            check_self_app(c2, alias, fname)
        }
        Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => {
            for x in xs {
                check_self_app(x, alias, fname)?;
            }
            Ok(())
        }
        Core::Match(s, arms) => {
            check_self_app(s, alias, fname)?;
            for (_, _, b) in arms {
                check_self_app(b, alias, fname)?;
            }
            Ok(())
        }
        Core::Proj(e, _) | Core::Lam(_, e) => check_self_app(e, alias, fname),
    }
}

fn check_bound(c: &Core, bound: &mut BTreeSet<u32>, fname: &str) -> Result<(), Diag> {
    match c {
        Core::Num(_) | Core::Flo(_) => Ok(()),
        Core::Var(v) => {
            if bound.contains(v) {
                Ok(())
            } else {
                Err(Diag::new(0, format!("clone discipline: unbound variable v{} in function '{}'", v, fname)))
            }
        }
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
            check_bound(a, bound, fname)?;
            check_bound(b, bound, fname)
        }
        Core::If(c1, t, e) => {
            check_bound(c1, bound, fname)?;
            check_bound(t, bound, fname)?;
            check_bound(e, bound, fname)
        }
        Core::Let(v, r, b) => {
            check_bound(r, bound, fname)?;
            bound.insert(*v);
            check_bound(b, bound, fname)
        }
        Core::Call(_, args) | Core::Ctor(_, args) | Core::Tuple(args) | Core::Reuse(_, _, args) | Core::Prim(_, args) => {
            for a in args {
                check_bound(a, bound, fname)?;
            }
            Ok(())
        }
        Core::Match(s, arms) => {
            check_bound(s, bound, fname)?;
            for (_, binders, body) in arms {
                for b in binders {
                    bound.insert(*b);
                }
                check_bound(body, bound, fname)?;
            }
            Ok(())
        }
        Core::Proj(e, _) => check_bound(e, bound, fname),
        Core::Lam(x, b) => {
            let fresh = bound.insert(*x);
            check_bound(b, bound, fname)?;
            if fresh {
                bound.remove(x);
            }
            Ok(())
        }
        Core::App(f, a) => {
            check_bound(f, bound, fname)?;
            check_bound(a, bound, fname)
        }
    }
}
