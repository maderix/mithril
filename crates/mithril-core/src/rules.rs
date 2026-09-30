//! The interaction rule table — one implementation for both times. The
//! compile-time reducer (`mithril-net`) and the runtime engine
//! (`mithril-rt`) call `process` on their own `Cells` with their own `Prog`
//! (what a call unfolds into, how a builtin computes, what to do with an
//! op that cannot compute yet).
//!
//! `process` dispatches one popped redex:
//!
//! - REF–anything (but ERA/REF): the program decides (`Prog::unfold`):
//!   splice the body, spawn compiled code, or keep the call.
//! - ERA–REF: erase the closure without unfolding (dead branch).
//! - Var on either side: pure wiring (`link`), NOT a rewrite — this is the
//!   static gate: a rule only ever fires between two non-Var ports, i.e.
//!   once every operand the rule needs has actually been produced.
//! - ERA–anything: erase.
//! - APP–LAM (beta), OP–value (compute, with an operand-swap half-step
//!   when the other operand is still in flight), SWI–NUM (select branch,
//!   erase the other), MAT–constructor (select arm / tuple projection),
//!   DUP–value (copy; a lambda HVM-style: two lambdas, body dup, params
//!   joined by a same-label dup acting as the superposition), DUP–DUP
//!   (same label annihilate, different commute), DUP–{OP, APP, SWI, MAT}
//!   (commute: the consumer passes through the superposition).
//! - Every other pair is an ICE naming the pair.

use crate::agents::*;
use crate::port::{Port, Tag};

/// What a `Mat` agent selects on.
pub enum MatMeta<'a> {
    /// tuple projection of field `i`
    Proj(usize),
    /// constructor tag per arm, in arm order
    Arms(&'a [u16]),
}

/// The program-specific half of the rules.
pub trait Prog<C: Cells> {
    /// REF rule: `r` met `other` (never an Era, never a Ref). Returns the
    /// rewrites performed (0 when the call is kept as a call).
    fn unfold(&self, c: &mut C, r: Port, other: Port) -> u64;
    /// Whether `r` is a closure (a lifted branch/arm carrying its captures,
    /// applied by SWI/MAT) rather than a saturated call: a Dup copies it
    /// instead of sharing a result.
    fn is_closure(&self, r: Port) -> bool;
    fn mat_meta(&self, mid: u16) -> MatMeta<'_>;
    /// OP rule: compute `code` on two produced operands; `None` when it
    /// cannot (a division by zero, an opaque builtin at compile time).
    fn compute(&self, c: &mut C, code: u16, x: Port, y: Port) -> Option<Port>;
    /// An op that could not compute: `op`'s cell holds `[x, ret]`, `y` is
    /// the other operand. Compile time keeps it residual; runtime cannot.
    fn park_op(&self, c: &mut C, op: Port, y: Port);
    /// Value forms only the runtime has (arrays, unboxed constructors).
    fn is_ext_value(&self, _p: Port) -> bool {
        false
    }
    fn copy_ext(&self, _c: &mut C, p: Port) -> Port {
        panic!("ICE: no copy for value {:?}", p.tag())
    }
    fn erase_ext(&self, _c: &mut C, p: Port) {
        panic!("ICE: no erasure for value {:?}", p.tag())
    }
    /// Constructor tag and fields of an ext value met by a `Mat`.
    fn ext_ctor(&self, _c: &mut C, p: Port) -> (u16, Vec<Port>) {
        panic!("ICE: no match on value {:?}", p.tag())
    }
    /// A value met a continuation (runtime only): hand it over.
    fn deliver(&self, _c: &mut C, kont: Port, _val: Port) {
        panic!("ICE: continuation {:?} at compile time", kont.tag())
    }
}

/// Connect two ports. Wire cells hold the first arrival in slot 0; the
/// second arrival takes it (freeing the cell) and the two ports meet. Two
/// non-Var ports meeting becomes a redex. Never fires a rule itself.
pub fn link<C: Cells>(c: &mut C, a: Port, b: Port) {
    let (mut a, mut b) = (a, b);
    loop {
        if a.tag() != Tag::Var {
            if b.tag() != Tag::Var {
                c.push_redex(a, b);
                return;
            }
            std::mem::swap(&mut a, &mut b);
        }
        let w = a.payload() as u32;
        let cell = c.cell(w);
        if cell[0] == EMPTY.0 {
            c.set(w, 0, b);
            return;
        }
        c.free_cell(w);
        a = Port(cell[0]);
        // retry linking the stored port against b
        std::mem::swap(&mut a, &mut b);
    }
}

/// Follow filled wires (freeing them) until a non-Var port or an unfilled
/// wire end is reached.
pub fn resolve<C: Cells>(c: &mut C, mut p: Port) -> Port {
    while p.tag() == Tag::Var {
        let w = p.payload() as u32;
        let cell = c.cell(w);
        if cell[0] == EMPTY.0 {
            return p;
        }
        c.free_cell(w);
        p = Port(cell[0]);
    }
    p
}

/// A net value as compiled code reads it: every constructor field,
/// transitively, resolved in place (filled wires followed and freed, the
/// value written into the field's slot). Returns the fields still waiting
/// on an unfilled wire as (cell, slot, wire end); empty when the value is
/// settled. Compiled code must not read a value until it is.
pub fn settle<C: Cells>(c: &mut C, v: Port) -> Vec<(u32, usize, Port)> {
    let with_fields = |p: Port| p.tag() == Tag::Con && p.con_arity() > 0;
    let mut pending = Vec::new();
    let mut work: Vec<Port> = [v].into_iter().filter(|p| with_fields(*p)).collect();
    while let Some(p) = work.pop() {
        let cell = p.con_addr() as u32;
        // arity 2: two fields; 1: one; wider: a field and the rest of the
        // chain. Pushed last-first, so the head is visited before the tail
        // and the worklist grows with left nesting only.
        if p.con_arity() > 2 {
            work.extend([Port(c.cell(cell)[1])].into_iter().filter(|r| with_fields(*r)));
        }
        let slots: &[usize] = if p.con_arity() == 2 { &[1, 0] } else { &[0] };
        for &s in slots {
            let mut f = Port(c.cell(cell)[s]);
            if f.tag() == Tag::Var {
                f = resolve(c, f);
                c.set(cell, s, f);
            }
            if f.tag() == Tag::Var {
                pending.push((cell, s, f));
            } else if with_fields(f) {
                work.push(f);
            }
        }
    }
    pending
}

/// A fresh `label` dup fed by `p`: its two output wires.
fn fan<C: Cells>(c: &mut C, label: u32, p: Port) -> (Port, Port) {
    let (w1, w2) = (wire(c), wire(c));
    let d = c.alloc(w1, w2);
    link(c, dup_port(d, label), p);
    (w1, w2)
}

fn is_value<C: Cells, P: Prog<C> + ?Sized>(prog: &P, p: Port) -> bool {
    matches!(p.tag(), Tag::Num | Tag::Flo | Tag::Con | Tag::Lam) || prog.is_ext_value(p)
}

/// Process one redex; returns the number of rewrites performed (0 for pure
/// wiring, 1 for a rule firing).
pub fn process<C: Cells, P: Prog<C> + ?Sized>(c: &mut C, prog: &P, a: Port, b: Port) -> u64 {
    let (ta, tb) = (a.tag(), b.tag());

    // REF first: a Ref against a Var must unfold, never park in the wire.
    if ta == Tag::Ref || tb == Tag::Ref {
        let (r, other) = if ta == Tag::Ref { (a, b) } else { (b, a) };
        if other.tag() == Tag::Ref {
            panic!("ICE: no interaction rule for Ref–Ref (two producers head-on)");
        }
        if other.tag() == Tag::Era {
            for p in list_collect(c, ref_head(r)) {
                link(c, era(), p);
            }
            return 1;
        }
        if other.tag() == Tag::Dup && prog.is_closure(r) {
            // DUP–closure: two closures over dup'd captures
            let (o1, o2) = take(c, dup_addr(other));
            let (ra, rb) = copy_closure(c, r, dup_label(other));
            link(c, ra, o1);
            link(c, rb, o2);
            return 1;
        }
        return prog.unfold(c, r, other);
    }

    // Wiring: the static gate. No rule fires across an unproduced value.
    if ta == Tag::Var || tb == Tag::Var {
        link(c, a, b);
        return 0;
    }

    // A produced value meets its continuation (runtime): delivered.
    if ta == Tag::Kont || tb == Tag::Kont {
        let (k, v) = if ta == Tag::Kont { (a, b) } else { (b, a) };
        assert!(is_value(prog, v), "ICE: continuation met a {:?}, not a value", v.tag());
        prog.deliver(c, k, v);
        return 1;
    }

    if ta == Tag::Era || tb == Tag::Era {
        let other = if ta == Tag::Era { b } else { a };
        era_value(c, prog, other);
        return 1;
    }

    match (ta, tb) {
        (Tag::App, Tag::Lam) => beta(c, a, b),
        (Tag::Lam, Tag::App) => beta(c, b, a),
        (Tag::Op, _) if is_value(prog, b) => op_rule(c, prog, a, b),
        (_, Tag::Op) if is_value(prog, a) => op_rule(c, prog, b, a),
        (Tag::Swi, Tag::Num) => swi_rule(c, a, b),
        (Tag::Num, Tag::Swi) => swi_rule(c, b, a),
        (Tag::Mat, _) if is_value(prog, b) => mat_rule(c, prog, a, b),
        (_, Tag::Mat) if is_value(prog, a) => mat_rule(c, prog, b, a),
        (Tag::Dup, Tag::Dup) => dup_dup(c, a, b),
        (Tag::Dup, _) if is_value(prog, b) => dup_rule(c, prog, a, b),
        (_, Tag::Dup) if is_value(prog, a) => dup_rule(c, prog, b, a),
        (Tag::Dup, Tag::Op) | (Tag::Dup, Tag::App) | (Tag::Dup, Tag::Swi) | (Tag::Dup, Tag::Mat) => dup_commute(c, a, b),
        (Tag::Op, Tag::Dup) | (Tag::App, Tag::Dup) | (Tag::Swi, Tag::Dup) | (Tag::Mat, Tag::Dup) => dup_commute(c, b, a),
        (x, y) => panic!("ICE: no interaction rule for {:?}–{:?}", x, y),
    }
    1
}

/// ERA–anything: consume and erase the value/agent `p`.
fn era_value<C: Cells, P: Prog<C> + ?Sized>(c: &mut C, prog: &P, p: Port) {
    match p.tag() {
        Tag::Era | Tag::Num => {}
        Tag::Flo => c.free_cell(p.payload() as u32),
        Tag::Con => {
            for f in con_collect(c, p) {
                link(c, era(), f);
            }
        }
        Tag::Dup | Tag::Lam | Tag::App | Tag::Op | Tag::Swi | Tag::Mat => {
            // slot 0 (a ret for Swi/Mat); slot 1 the Swi's arms cell, the
            // Mat's arm list
            let (x, y) = take(c, addr_of(p));
            link(c, era(), x);
            match p.tag() {
                Tag::Swi => {
                    let (t, e) = take(c, y.payload() as u32);
                    link(c, era(), t);
                    link(c, era(), e);
                }
                Tag::Mat => {
                    for r in list_collect(c, y) {
                        link(c, era(), r);
                    }
                }
                _ => link(c, era(), y),
            }
        }
        _ if prog.is_ext_value(p) => prog.erase_ext(c, p),
        t => panic!("ICE: ERA–{:?} has no erasure", t),
    }
}

/// APP–LAM beta: arg meets param, body meets ret.
fn beta<C: Cells>(c: &mut C, app: Port, lam: Port) {
    debug_assert!(app.payload() != lam.payload(), "ICE: beta on one cell");
    let (arg, ret) = take(c, app.payload() as u32);
    let (param, body) = take(c, lam.payload() as u32);
    link(c, arg, param);
    link(c, ret, body);
}

/// OP–value: compute if the other operand has been produced, otherwise
/// store this one and re-arm the op (flipped) against the missing operand.
fn op_rule<C: Cells, P: Prog<C> + ?Sized>(c: &mut C, prog: &P, op: Port, val: Port) {
    let addr = op_addr(op);
    let code = op_code(op);
    let cell = c.cell(addr);
    let other = resolve(c, Port(cell[0]));
    if other.tag() == Tag::Var {
        c.set(addr, 0, val);
        link(c, op_port(addr, code | OP_FLIP), other);
        return;
    }
    if other.tag() == Tag::Dup {
        // OP–SUP: the other operand is a superposition (a copied closure's
        // parameter): the op splits into one per side, the produced
        // operand copied to both, the result superposed on the ret
        let label = dup_label(other);
        let (s1, s2) = take(c, dup_addr(other));
        c.free_cell(addr);
        let (v1, v2) = fan(c, label, val);
        let (r1, r2) = fan(c, label, Port(cell[1]));
        let a1 = c.alloc(s1, r1);
        let a2 = c.alloc(s2, r2);
        link(c, op_port(a1, code), v1);
        link(c, op_port(a2, code), v2);
        return;
    }
    if !is_value(prog, other) {
        panic!("ICE: Op operand has non-value tag {:?}", other.tag());
    }
    let (x, y) = if code & OP_FLIP != 0 { (other, val) } else { (val, other) };
    match prog.compute(c, code & 0xFF, x, y) {
        Some(r) => {
            let ret = Port(cell[1]);
            c.free_cell(addr);
            link(c, r, ret);
        }
        None => {
            c.set(addr, 0, x);
            prog.park_op(c, op_port(addr, code & !OP_FLIP), y);
        }
    }
}

/// SWI–NUM: fire the taken branch closure at the return port, erase the
/// other. Branch closures are unfired `Ref`s, so the untaken branch's code
/// was never built — this is where laziness (and loop termination) lives.
fn swi_rule<C: Cells>(c: &mut C, swi: Port, num: Port) {
    let (ret, arms) = take(c, swi.payload() as u32);
    let (t, e) = take(c, arms.payload() as u32);
    let (taken, dead) = if num.as_i64() != 0 { (t, e) } else { (e, t) };
    link(c, era(), dead);
    c.push_redex(taken, ret);
}

/// MAT–constructor: select the arm whose ctor tag matches the scrutinee,
/// prepend the constructor's fields to the arm closure's captured args and
/// fire it; erase the other arms. A projection is the 1-way special case.
fn mat_rule<C: Cells, P: Prog<C> + ?Sized>(c: &mut C, prog: &P, mat: Port, val: Port) {
    let mid = mat_id(mat);
    let (ret, arms) = take(c, mat_addr(mat));
    let (ct, fields) = if val.tag() == Tag::Con { (val.con_tag(), con_collect(c, val)) } else { prog.ext_ctor(c, val) };
    match prog.mat_meta(mid) {
        MatMeta::Proj(i) => {
            assert_eq!(ct, CTAG_TUPLE, "ICE: projection on non-tuple constructor {}", ct);
            assert!(i < fields.len(), "ICE: projection index {} out of bounds ({})", i, fields.len());
            for (j, f) in fields.into_iter().enumerate() {
                if j == i {
                    link(c, f, ret);
                } else {
                    link(c, era(), f);
                }
            }
        }
        MatMeta::Arms(tags) => {
            let refs = list_collect(c, arms);
            debug_assert_eq!(refs.len(), tags.len());
            let j = tags
                .iter()
                .position(|t| *t == ct)
                .unwrap_or_else(|| panic!("ICE: match has no arm for ctor tag {} (non-exhaustive at net level)", ct));
            for (i, r) in refs.iter().enumerate() {
                if i != j {
                    link(c, era(), *r);
                }
            }
            let rj = refs[j];
            assert_eq!(rj.tag(), Tag::Ref, "ICE: match arm slot holds a {:?}, not a closure", rj.tag());
            let mut args = fields; // ctor fields = pattern binders
            args.extend(list_collect(c, ref_head(rj))); // then captured frees
            let head = list_alloc(c, &args);
            c.push_redex(ref_port(head, ref_entry(rj)), ret);
        }
    }
}

/// DUP–value: copy. NUM is free to copy; FLO reboxes; CON copies one chain
/// of cells and pushes DUPs onto its fields (lazy recursion); LAM is
/// HVM-style (two lams, body dup, params joined by a same-label dup acting
/// as the superposition). Copies carry the copier's label.
fn dup_rule<C: Cells, P: Prog<C> + ?Sized>(c: &mut C, prog: &P, dup: Port, val: Port) {
    let label = dup_label(dup);
    let (o1, o2) = take(c, dup_addr(dup));
    match val.tag() {
        Tag::Num => {
            link(c, val, o1);
            link(c, val, o2);
        }
        Tag::Flo => {
            let bits = c.cell(val.payload() as u32);
            let copy = c.alloc(Port(bits[0]), Port(bits[1]));
            link(c, val, o1);
            link(c, Port::new(Tag::Flo, copy as u64), o2);
        }
        Tag::Con => {
            let ctag = val.con_tag();
            let fields = con_collect(c, val);
            let mut fa = Vec::with_capacity(fields.len());
            let mut fb = Vec::with_capacity(fields.len());
            for f in fields {
                let (w1, w2) = fan(c, label, f);
                fa.push(w1);
                fb.push(w2);
            }
            let ca = con_alloc(c, ctag, &fa);
            let cb = con_alloc(c, ctag, &fb);
            link(c, ca, o1);
            link(c, cb, o2);
        }
        Tag::Lam => {
            let (l1, l2) = copy_lam(c, val.payload() as u32, label);
            link(c, l1, o1);
            link(c, l2, o2);
        }
        _ => {
            let copy = prog.copy_ext(c, val);
            link(c, val, o1);
            link(c, copy, o2);
        }
    }
}

/// The DUP–closure copy of an arm/branch closure `r`: two Refs to the
/// same entry over dup'd captures (the copies carry the copier's label,
/// like a constructor's fields).
pub fn copy_closure<C: Cells>(c: &mut C, r: Port, label: u32) -> (Port, Port) {
    let caps = list_collect(c, ref_head(r));
    let (mut ca, mut cb) = (Vec::new(), Vec::new());
    for p in caps {
        let (w1, w2) = fan(c, label, p);
        ca.push(w1);
        cb.push(w2);
    }
    let entry = ref_entry(r);
    (ref_port(list_alloc(c, &ca), entry), ref_port(list_alloc(c, &cb), entry))
}

/// The DUP–LAM copy of the closure in cell `l`: two lambdas, their bodies
/// joined by a `label` dup on the old body wire, their parameters by a
/// same-label dup acting as the superposition on the old parameter wire.
/// The first copy keeps cell `l` (a port to it stays a valid closure:
/// compiled code shares closures this way), the second is fresh.
pub fn copy_lam<C: Cells>(c: &mut C, l: u32, label: u32) -> (Port, Port) {
    let cl = c.cell(l);
    let (wp1, wp2, wb1, wb2) = (wire(c), wire(c), wire(c), wire(c));
    let db = c.alloc(wb1, wb2);
    link(c, dup_port(db, label), Port(cl[1])); // copy the body
    let su = c.alloc(wp1, wp2);
    link(c, dup_port(su, label), Port(cl[0])); // param superposition
    c.set(l, 0, wp1);
    c.set(l, 1, wb1);
    let l2 = c.alloc(wp2, wb2);
    (Port::new(Tag::Lam, l as u64), Port::new(Tag::Lam, l2 as u64))
}

/// DUP–{OP, APP, SWI, MAT}: commute. The dup here is a superposition (its
/// principal port faces a consumer, not a value: a duplicated lambda's
/// parameter, or a shared value's consumer met before the value), so the
/// consumer is copied once per side and every other port of it gets a
/// same-label dup joining the two copies' ports: the consumer passes
/// through the dup. What each copy then computes is its own; what they
/// share (an operand already produced, a call's result) is shared.
fn dup_commute<C: Cells>(c: &mut C, dup: Port, agent: Port) {
    let label = dup_label(dup);
    let (o1, o2) = take(c, dup_addr(dup));
    // a same-label dup whose outputs are fresh wires; its input is `p`.
    // An arm closure (a Ref, a value) is copied right away: arm slots
    // always hold closures, never wires a copy will arrive on later
    let split = |c: &mut C, p: Port| -> (Port, Port) {
        if p.tag() == Tag::Ref {
            return copy_closure(c, p, label);
        }
        fan(c, label, p)
    };
    match agent.tag() {
        Tag::Op => {
            let code = op_code(agent);
            let (x, r) = take(c, op_addr(agent));
            let (x1, x2) = split(c, x);
            let (r1, r2) = split(c, r);
            let a1 = c.alloc(x1, r1);
            let a2 = c.alloc(x2, r2);
            link(c, op_port(a1, code), o1);
            link(c, op_port(a2, code), o2);
        }
        Tag::App => {
            let (x, r) = take(c, agent.payload() as u32);
            let (x1, x2) = split(c, x);
            let (r1, r2) = split(c, r);
            let a1 = c.alloc(x1, r1);
            let a2 = c.alloc(x2, r2);
            link(c, Port::new(Tag::App, a1 as u64), o1);
            link(c, Port::new(Tag::App, a2 as u64), o2);
        }
        Tag::Swi => {
            let (r, arms) = take(c, agent.payload() as u32);
            let (t, e) = take(c, arms.payload() as u32);
            let (r1, r2) = split(c, r);
            let (t1, t2) = split(c, t);
            let (e1, e2) = split(c, e);
            let arms1 = c.alloc(t1, e1);
            let arms2 = c.alloc(t2, e2);
            let n1 = c.alloc(r1, Port::new(Tag::Ext, arms1 as u64));
            let n2 = c.alloc(r2, Port::new(Tag::Ext, arms2 as u64));
            link(c, Port::new(Tag::Swi, n1 as u64), o1);
            link(c, Port::new(Tag::Swi, n2 as u64), o2);
        }
        Tag::Mat => {
            let id = mat_id(agent);
            let (r, list) = take(c, mat_addr(agent));
            let (r1, r2) = split(c, r);
            let arms = list_collect(c, list);
            let (mut l1, mut l2) = (Vec::new(), Vec::new());
            for r in arms {
                let (a1, a2) = split(c, r);
                l1.push(a1);
                l2.push(a2);
            }
            let h1 = list_alloc(c, &l1);
            let h2 = list_alloc(c, &l2);
            let n1 = c.alloc(r1, h1);
            let n2 = c.alloc(r2, h2);
            link(c, mat_port(n1, id), o1);
            link(c, mat_port(n2, id), o2);
        }
        t => panic!("ICE: Dup–{:?} has no commutation", t),
    }
}

/// DUP–DUP: the same label (the two halves of one copy meeting again)
/// annihilate, wires cross-connect; different labels commute, each dup
/// passing through the other (four dups, the copies of `a` carry b's
/// label and vice versa).
fn dup_dup<C: Cells>(c: &mut C, a: Port, b: Port) {
    let (la, lb) = (dup_label(a), dup_label(b));
    let (a0, a1) = take(c, dup_addr(a));
    let (b0, b1) = take(c, dup_addr(b));
    if la == lb {
        link(c, a0, b0);
        link(c, a1, b1);
        return;
    }
    let w: Vec<Port> = (0..4).map(|_| wire(c)).collect();
    // a's outputs each receive a b-labelled dup; b's outputs an a-labelled one
    let n1 = c.alloc(w[0], w[1]);
    let n2 = c.alloc(w[2], w[3]);
    let m1 = c.alloc(w[0], w[2]);
    let m2 = c.alloc(w[1], w[3]);
    link(c, dup_port(n1, lb), a0);
    link(c, dup_port(n2, lb), a1);
    link(c, dup_port(m1, la), b0);
    link(c, dup_port(m2, la), b1);
}
