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
//!   `[param, body]` / `[arg, ret]` (closures: `Core::Lam`/`Core::App`).
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
pub mod expose;
pub mod known;
pub mod reduce;
pub mod residual;
pub mod rules;

use mithril_core::net::Net;
use mithril_front::ast::{BinOp, CmpOp};
use mithril_front::core::{Core, CoreModule, Prim, UNREACHABLE_CTOR};
use mithril_front::Diag;
use std::collections::{BTreeMap, BTreeSet};

/// The rule-firing budget of compile-time reduction (the residual keeps the rest).
pub const REDUCE_FUEL: u64 = 1 << 20;

pub use build::{build, root_port};
pub use known::specialize;
pub use reduce::{readback, reduce, SpecReport};
pub use rules::link;

// ---- opcode space (Op payload low 16 bits) ----

/// Opcodes from here up are the builtins (`Core::Prim`), strict agents like
/// the arithmetic ops: binary32 ops fold on numbers; array ops are opaque at
/// compile time (arrays are runtime heap values) and always stay residual.
/// A unary builtin rides a binary Op with a `Num(0)` second operand; the
/// ternary `array_set(a, i, v)` is `ARRSET(a, ARR_PAIR(i, v))`.

/// Builtins by opcode (index + PRIM_BASE) and whether each is unary.
const PRIMS: [(Prim, bool); 13] = [
    (Prim::F32Add, false), (Prim::F32Sub, false), (Prim::F32Mul, false), (Prim::F32Div, false),
    (Prim::F32Sqrt, true), (Prim::F32Lt, false), (Prim::F32FromU32, true), (Prim::F32ToU32, true),
    (Prim::ArrNew, false), (Prim::ArrGet, false), (Prim::ArrLen, true), (Prim::ArrSet, false),
    (Prim::F32Le, false),
];

// the pairing pseudo-op sits right after the builtins; the runtime
// dispatchers (rt template.rs, engine.cu) number the codes the same way
const _: () = assert!(ARR_PAIR == PRIM_BASE + PRIMS.len() as u16);

pub(crate) fn prim_code(p: Prim) -> u16 {
    PRIM_BASE + PRIMS.iter().position(|(q, _)| *q == p).unwrap() as u16
}

/// The builtin of an opcode and whether it is unary.
pub(crate) fn prim_of_code(code: u16) -> (Prim, bool) {
    *code.checked_sub(PRIM_BASE).and_then(|i| PRIMS.get(i as usize)).unwrap_or_else(|| panic!("ICE: opcode {} is not a builtin", code))
}

/// BinOp opcodes are the declaration order (`BinOp::ALL`), CmpOp's 16 up.
pub fn opcode_bin(op: BinOp) -> u16 {
    op as u16
}

pub(crate) fn opcode_cmp(op: CmpOp) -> u16 {
    16 + op as u16
}

pub use mithril_core::agents::*;
pub use mithril_core::lower::{count_uses, instantiate, ClosureSpec, Entry, MatchMeta, NExpr, ARR_PAIR, PRIM_BASE};

/// A fresh Dup label (a new sharing site).
pub fn fresh_label(net: &mut Net) -> u32 {
    mithril_core::agents::Cells::fresh_label(net)
}

// ---- derived program: lambda-lifted Core ----

/// What the REF-unfold rule does with a call to a real function.
#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    /// evaluation: unfold every call
    Eval,
    /// specialization: unfold a call only where `NetProg::inline` allows;
    /// other calls stay residual
    Specialize,
}

pub struct NetProg {
    pub entries: Vec<Entry>,
    pub metas: Vec<MatchMeta>,
    /// real functions are entries 0..nfns; lifted branches/arms follow
    pub nfns: usize,
    pub mode: Mode,
    /// per real function: unfold its calls during specialization (it is
    /// on no call cycle and small, i.e. inlining it cannot recurse)
    pub inline: Vec<bool>,
    /// entries registered by compiled code (`add_entry`)
    pub closures: Vec<usize>,
}

impl NetProg {
    /// Deterministically derived from the module alone, so `build` and
    /// `reduce` agree on entry ids without storing the table in the `Net`.
    pub fn new(m: &CoreModule) -> NetProg {
        let nfns = m.fns.len();
        let mut prog = NetProg { entries: Vec::new(), metas: Vec::new(), nfns, mode: Mode::Eval, inline: vec![false; nfns], closures: Vec::new() };
        for f in &m.fns {
            prog.entries.push(Entry { params: (0..f.arity as u32).collect(), body: NExpr::Num(0) });
        }
        for (i, f) in m.fns.iter().enumerate() {
            let body = lower(&f.body, &mut prog);
            prog.entries[i].body = body;
        }
        // unfold a call-free function whose body is small (a call-free body
        // cannot recurse); larger or calling functions stay calls unless
        // their arguments are all known (then they are evaluated)
        prog.inline = m
            .fns
            .iter()
            .enumerate()
            .map(|(f, fd)| f as u32 != m.main && fd.body.size() <= INLINE_MAX && !fd.body.any(&mut |e| matches!(e, Core::Call(..)).then_some(true)))
            .collect();
        prog
    }
}

/// Inlining budget for acyclic callees during specialization (Core nodes).
const INLINE_MAX: usize = 96;

impl NetProg {
    /// Lower `body` as a new entry over `params` (its free variables, in
    /// this order): how compiled code registers the closures it builds.
    pub fn add_entry(&mut self, params: Vec<u32>, body: &Core) -> u16 {
        let id = lift(self, params, body);
        self.closures.push(id as usize);
        id
    }

    /// Whether entry `e` was registered by compiled code (`add_entry`).
    pub fn closure_entry(&self, e: usize) -> bool {
        self.closures.contains(&e)
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
    let caps: Vec<u32> = body.free_vars().into_iter().filter(|v| !binders.contains(v)).collect();
    let mut params = binders.to_vec();
    params.extend(caps.iter().copied());
    let entry = lift(prog, params, body);
    ClosureSpec { entry, caps }
}

pub(crate) fn ctag_of(cid: u32) -> u16 {
    if cid == UNREACHABLE_CTOR {
        CTAG_UNREACHABLE
    } else {
        assert!(cid < CTAG_UNREACHABLE as u32, "ICE: ctor id {} exceeds the 12-bit cell tag space", cid);
        cid as u16
    }
}

/// The inverse of `ctag_of`.
pub(crate) fn cid_of(t: u16) -> u32 {
    if t == CTAG_UNREACHABLE { UNREACHABLE_CTOR } else { t as u32 }
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
                let fvs: BTreeSet<u32> = f.free_vars().into_iter().map(|u| canon(u, alias)).collect();
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
        _ => c.kids().into_iter().try_for_each(|k| check_self_app(k, alias, fname)),
    }
}

fn check_bound(c: &Core, bound: &mut BTreeSet<u32>, fname: &str) -> Result<(), Diag> {
    match c {
        Core::Var(v) if !bound.contains(v) => Err(Diag::new(0, format!("clone discipline: unbound variable v{} in function '{}'", v, fname))),
        Core::Let(v, r, b) => {
            check_bound(r, bound, fname)?;
            bound.insert(*v);
            check_bound(b, bound, fname)
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
        Core::Lam(x, b) => {
            let fresh = bound.insert(*x);
            check_bound(b, bound, fname)?;
            if fresh {
                bound.remove(x);
            }
            Ok(())
        }
        _ => c.kids().into_iter().try_for_each(|k| check_bound(k, bound, fname)),
    }
}
