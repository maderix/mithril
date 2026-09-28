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
//! - `Dup(a,l)` — payload `addr:40|label:16` (single label class, 0);
//!   cell = the two copy targets.
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
pub mod rules;

use mithril_core::net::Net;
use mithril_core::port::{Port, Tag};
use mithril_front::ast::{BinOp, CmpOp};
use mithril_front::core::{Core, CoreModule, UNREACHABLE_CTOR};
use mithril_front::Diag;
use std::collections::BTreeSet;

pub use build::{build, root_port};
pub use reduce::{readback, reduce};

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
pub(crate) fn wire(net: &mut Net) -> Port {
    let w = net.alloc(EMPTY, EMPTY);
    Port::new(Tag::Var, w as u64)
}

// ---- opcode space (Op payload low 16 bits) ----

/// "Operands swapped" flag: slot 0 holds the *first* operand.
pub(crate) const OP_FLIP: u16 = 1 << 8;

pub(crate) fn opcode_bin(op: BinOp) -> u16 {
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

pub(crate) fn op_port(addr: u32, code: u16) -> Port {
    Port::new(Tag::Op, ((addr as u64) << 16) | code as u64)
}
pub(crate) fn op_addr(p: Port) -> u32 {
    (p.payload() >> 16) as u32
}
pub(crate) fn op_code(p: Port) -> u16 {
    (p.payload() & 0xFFFF) as u16
}

pub(crate) fn dup_port(addr: u32) -> Port {
    Port::new(Tag::Dup, (addr as u64) << 16) // label 0: single label class
}
pub(crate) fn dup_addr(p: Port) -> u32 {
    (p.payload() >> 16) as u32
}
pub(crate) fn dup_label(p: Port) -> u16 {
    (p.payload() & 0xFFFF) as u16
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
pub(crate) fn ref_port(head: Port, entry: u16) -> Port {
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

pub(crate) fn list_alloc(net: &mut Net, items: &[Port]) -> Port {
    let mut head = EMPTY;
    for &it in items.iter().rev() {
        let a = net.alloc(it, head);
        head = Port::new(Tag::Ext, a as u64);
    }
    head
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

pub(crate) fn con_alloc(net: &mut Net, ctag: u16, fields: &[Port]) -> Port {
    let n = fields.len();
    assert!(n < 16, "ICE: constructor arity {} exceeds the 4-bit cell arity", n);
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
            Port::con(a as u64, ctag, n as u8)
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

pub(crate) struct NetProg {
    pub entries: Vec<Entry>,
    pub metas: Vec<MatchMeta>,
}

impl NetProg {
    /// Deterministically derived from the module alone, so `build` and
    /// `reduce` agree on entry ids without storing the table in the `Net`.
    pub(crate) fn new(m: &CoreModule) -> NetProg {
        let mut prog = NetProg { entries: Vec::new(), metas: Vec::new() };
        for f in &m.fns {
            prog.entries.push(Entry { params: (0..f.arity as u32).collect(), body: NExpr::Num(0) });
        }
        for (i, f) in m.fns.iter().enumerate() {
            let body = lower(&f.body, &mut prog);
            prog.entries[i].body = body;
        }
        prog
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
        // never lowered: modules with builtins skip compile-time reduction
        // (see `uses_prims`); a placeholder keeps the entry table total
        Core::Prim(..) => NExpr::Num(0),
    }
}

/// Whether any function uses a builtin. Arrays are runtime values with
/// in-place updates, so such a module is not reduced at compile time.
pub fn uses_prims(m: &CoreModule) -> bool {
    fn has(e: &Core) -> bool {
        match e {
            Core::Prim(..) => true,
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => false,
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => has(a) || has(b),
            Core::If(a, b, c) => has(a) || has(b) || has(c),
            Core::Let(_, r, b) => has(r) || has(b),
            Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) => xs.iter().any(has),
            Core::Match(s, arms) => has(s) || arms.iter().any(|(_, _, b)| has(b)),
            Core::Proj(a, _) => has(a),
        }
    }
    m.fns.iter().any(|f| has(&f.body))
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
    }
}

// ---- clone discipline ----

/// v1 Core is first-order (desugar rejects lambdas with a Diag before the
/// net tier), so every variable a `build`-inserted DUP duplicates is a
/// *data* value — always sound to copy. This check verifies that
/// structural precondition (every variable use is bound by a param, `Let`
/// or match binder, i.e. every DUP target is a value produced in the same
/// function) and returns `Ok`. When closures land in Core, this is where a
/// lambda body using its parameter more than once under an enclosing DUP
/// is rejected, naming the variable.
pub fn check_clone_discipline(m: &CoreModule) -> Result<(), Diag> {
    for f in &m.fns {
        let mut bound: BTreeSet<u32> = (0..f.arity as u32).collect();
        check_bound(&f.body, &mut bound, &f.name)?;
    }
    Ok(())
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
    }
}
