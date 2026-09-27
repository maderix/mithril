//! The interaction rule table. `process` dispatches one popped redex:
//!
//! - REF–anything (but ERA/REF): unfold the entry's body, fuel-gated —
//!   this one rule subsumes the brief's APP–REF and MAT–REF (the partner
//!   is wherever the unfolded body's result flows).
//! - ERA–REF: erase the closure without unfolding (dead SWI/MAT branch).
//! - Var on either side: pure wiring (`link`), NOT a rewrite — this is the
//!   static gate: a rule only ever fires between two non-Var ports, i.e.
//!   once every operand the rule needs has actually been produced.
//! - ERA–anything: erase.
//! - APP–LAM (beta), OP–NUM/FLO (fold, with an operand-swap half-step when
//!   the other operand is still in flight), SWI–NUM (select branch, erase
//!   the other), MAT–CON (select arm / tuple projection), DUP–NUM/FLO/CON/
//!   LAM (copy / lazy copy), DUP–DUP same label (annihilate).
//! - Every other pair is a compile-time ICE naming the pair.

use crate::build::instantiate;
use crate::{
    con_alloc, con_collect, dup_addr, dup_label, dup_port, era, list_collect, mat_addr, mat_id,
    op_addr, op_code, op_port, ref_entry, ref_head, wire, MatchMeta, NetProg, CTAG_TUPLE, EMPTY,
    OP_FLIP,
};
use mithril_core::net::Net;
use mithril_core::port::{Port, Tag};

const MASK56: u64 = (1u64 << 56) - 1;

/// Connect two ports. Wire cells hold the first arrival in slot 0; the
/// second arrival takes it (freeing the cell) and the two ports meet. Two
/// non-Var ports meeting becomes a redex. Never fires a rule itself.
pub(crate) fn link(net: &mut Net, a: Port, b: Port) {
    let (mut a, mut b) = (a, b);
    loop {
        if a.tag() != Tag::Var {
            if b.tag() != Tag::Var {
                net.redexes.push((a, b));
                return;
            }
            std::mem::swap(&mut a, &mut b);
        }
        let w = a.payload() as u32;
        let c = net.cell(w);
        if c[0] == EMPTY.0 {
            net.set(w, 0, b);
            return;
        }
        net.free_cell(w);
        a = Port(c[0]);
        // retry linking the stored port against b
        std::mem::swap(&mut a, &mut b);
    }
}

/// Follow filled wires (freeing them) until a non-Var port or an unfilled
/// wire end is reached.
pub(crate) fn resolve(net: &mut Net, mut p: Port) -> Port {
    while p.tag() == Tag::Var {
        let w = p.payload() as u32;
        let c = net.cell(w);
        if c[0] == EMPTY.0 {
            return p;
        }
        net.free_cell(w);
        p = Port(c[0]);
    }
    p
}

/// Process one redex; returns the number of rewrites performed (0 for pure
/// wiring, 1 for a rule firing).
pub(crate) fn process(net: &mut Net, prog: &NetProg, a: Port, b: Port) -> u64 {
    let (ta, tb) = (a.tag(), b.tag());

    // REF first: a Ref against a Var must unfold, never park in the wire.
    if ta == Tag::Ref || tb == Tag::Ref {
        let (r, other) = if ta == Tag::Ref { (a, b) } else { (b, a) };
        if other.tag() == Tag::Ref {
            panic!("ICE: no interaction rule for Ref–Ref (two producers head-on)");
        }
        let args = list_collect(net, ref_head(r));
        if other.tag() == Tag::Era {
            for p in args {
                link(net, era(), p);
            }
        } else {
            instantiate(net, prog, ref_entry(r) as usize, args, other);
        }
        return 1;
    }

    // Wiring: the static gate. No rule fires across an unproduced value.
    if ta == Tag::Var || tb == Tag::Var {
        link(net, a, b);
        return 0;
    }

    if ta == Tag::Era || tb == Tag::Era {
        let other = if ta == Tag::Era { b } else { a };
        era_value(net, other);
        return 1;
    }

    match (ta, tb) {
        (Tag::App, Tag::Lam) => beta(net, a, b),
        (Tag::Lam, Tag::App) => beta(net, b, a),
        (Tag::Op, Tag::Num) | (Tag::Op, Tag::Flo) => op_rule(net, a, b),
        (Tag::Num, Tag::Op) | (Tag::Flo, Tag::Op) => op_rule(net, b, a),
        (Tag::Swi, Tag::Num) => swi_rule(net, a, b),
        (Tag::Num, Tag::Swi) => swi_rule(net, b, a),
        (Tag::Mat, Tag::Con) => mat_rule(net, prog, a, b),
        (Tag::Con, Tag::Mat) => mat_rule(net, prog, b, a),
        (Tag::Dup, Tag::Dup) => dup_dup(net, a, b),
        (Tag::Dup, Tag::Num) | (Tag::Dup, Tag::Flo) | (Tag::Dup, Tag::Con) | (Tag::Dup, Tag::Lam) => {
            dup_rule(net, a, b)
        }
        (Tag::Num, Tag::Dup) | (Tag::Flo, Tag::Dup) | (Tag::Con, Tag::Dup) | (Tag::Lam, Tag::Dup) => {
            dup_rule(net, b, a)
        }
        (x, y) => panic!("ICE: no interaction rule for {:?}–{:?}", x, y),
    }
    1
}

/// ERA–anything: consume and erase the value/agent `p`.
fn era_value(net: &mut Net, p: Port) {
    match p.tag() {
        Tag::Era | Tag::Num => {}
        Tag::Flo => net.free_cell(p.payload() as u32),
        Tag::Con => {
            for f in con_collect(net, p) {
                link(net, era(), f);
            }
        }
        Tag::Dup => {
            let d = dup_addr(p);
            let c = net.cell(d);
            net.free_cell(d);
            link(net, era(), Port(c[0]));
            link(net, era(), Port(c[1]));
        }
        Tag::Lam | Tag::App => {
            let a = p.payload() as u32;
            let c = net.cell(a);
            net.free_cell(a);
            link(net, era(), Port(c[0]));
            link(net, era(), Port(c[1]));
        }
        Tag::Op => {
            let a = op_addr(p);
            let c = net.cell(a);
            net.free_cell(a);
            link(net, era(), Port(c[0]));
            link(net, era(), Port(c[1]));
        }
        Tag::Swi => {
            let s = p.payload() as u32;
            let c = net.cell(s);
            net.free_cell(s);
            link(net, era(), Port(c[0])); // ret
            let s2 = Port(c[1]).payload() as u32;
            let c2 = net.cell(s2);
            net.free_cell(s2);
            link(net, era(), Port(c2[0]));
            link(net, era(), Port(c2[1]));
        }
        Tag::Mat => {
            let m = mat_addr(p);
            let c = net.cell(m);
            net.free_cell(m);
            link(net, era(), Port(c[0])); // ret
            for r in list_collect(net, Port(c[1])) {
                link(net, era(), r);
            }
        }
        t => panic!("ICE: ERA–{:?} has no erasure", t),
    }
}

/// APP–LAM beta: arg meets param, body meets ret.
fn beta(net: &mut Net, app: Port, lam: Port) {
    let ia = app.payload() as u32;
    let il = lam.payload() as u32;
    let ca = net.cell(ia);
    let cl = net.cell(il);
    net.free_cell(ia);
    net.free_cell(il);
    link(net, Port(ca[0]), Port(cl[0])); // arg -> param
    link(net, Port(ca[1]), Port(cl[1])); // ret <- body
}

/// OP–NUM/FLO: fold if the other operand has been produced, otherwise
/// store this one and re-arm the op (flipped) against the missing operand.
fn op_rule(net: &mut Net, op: Port, val: Port) {
    let addr = op_addr(op);
    let code = op_code(op);
    let c = net.cell(addr);
    let other = resolve(net, Port(c[0]));
    match other.tag() {
        Tag::Num | Tag::Flo => {
            let ret = Port(c[1]);
            net.free_cell(addr);
            let (x, y) = if code & OP_FLIP != 0 { (other, val) } else { (val, other) };
            let r = compute(net, code & 0xFF, x, y);
            link(net, r, ret);
        }
        Tag::Var => {
            net.set(addr, 0, val);
            link(net, op_port(addr, code | OP_FLIP), other);
        }
        t => panic!("ICE: Op operand has non-numeric tag {:?}", t),
    }
}

fn wrap56(v: i64) -> i64 {
    ((v as u64) << 8) as i64 >> 8
}

fn floor_div(a: i64, b: i64) -> i64 {
    let q = a.wrapping_div(b);
    let r = a.wrapping_rem(b);
    if r != 0 && (r < 0) != (b < 0) {
        q - 1
    } else {
        q
    }
}

fn py_mod(a: i64, b: i64) -> i64 {
    let r = a.wrapping_rem(b);
    if r != 0 && (r < 0) != (b < 0) {
        r + b
    } else {
        r
    }
}

fn cmp_result(code: u16, ord: std::cmp::Ordering) -> Port {
    use std::cmp::Ordering::*;
    let r = match code {
        16 => ord == Less,
        17 => ord != Greater,
        18 => ord == Greater,
        19 => ord != Less,
        20 => ord == Equal,
        21 => ord != Equal,
        _ => panic!("ICE: unknown cmp opcode {}", code),
    };
    Port::num(r as i64)
}

/// Boxed f64 helpers: the bit pattern is split across two `Num`-tagged
/// ports so `Net::dump` never sees an invalid tag byte.
pub(crate) fn flo_alloc(net: &mut Net, f: f64) -> Port {
    let bits = f.to_bits();
    let a = net.alloc(Port::new(Tag::Num, bits & MASK56), Port::new(Tag::Num, bits >> 56));
    Port::new(Tag::Flo, a as u64)
}

pub(crate) fn flo_bits(cell: [u64; 2]) -> u64 {
    (Port(cell[0]).payload()) | (Port(cell[1]).payload() << 56)
}

/// Numeric fold, mirroring `eval_core`'s semantics exactly (i56 wrapping
/// ints, f64 floats boxed in cells, comparisons producing 0/1).
fn compute(net: &mut Net, code: u16, x: Port, y: Port) -> Port {
    match (x.tag(), y.tag()) {
        (Tag::Num, Tag::Num) => {
            let (a, b) = (x.as_i64(), y.as_i64());
            if code >= 16 {
                return cmp_result(code, a.cmp(&b));
            }
            let r = match code {
                0 => a.wrapping_add(b),
                1 => a.wrapping_sub(b),
                2 => a.wrapping_mul(b),
                3 => a.wrapping_div(b),
                4 => floor_div(a, b),
                5 => py_mod(a, b),
                6 => a.wrapping_shl(b as u32),
                7 => a.wrapping_shr(b as u32),
                8 => a & b,
                9 => a | b,
                10 => a ^ b,
                _ => panic!("ICE: unknown int opcode {}", code),
            };
            Port::num(wrap56(r))
        }
        (Tag::Flo, Tag::Flo) => {
            let (ax, ay) = (x.payload() as u32, y.payload() as u32);
            let a = f64::from_bits(flo_bits(net.cell(ax)));
            let b = f64::from_bits(flo_bits(net.cell(ay)));
            net.free_cell(ax);
            net.free_cell(ay);
            if code >= 16 {
                let ord = a.partial_cmp(&b).expect("ICE: incomparable floats (NaN)");
                return cmp_result(code, ord);
            }
            let r = match code {
                0 => a + b,
                1 => a - b,
                2 => a * b,
                3 => a / b,
                _ => panic!("ICE: opcode {} not defined on floats", code),
            };
            flo_alloc(net, r)
        }
        (tx, ty) => panic!("ICE: Op fold on mixed operand tags {:?}/{:?}", tx, ty),
    }
}

/// SWI–NUM: fire the taken branch closure at the return port, erase the
/// other. Branch closures are unfired `Ref`s, so the untaken branch's code
/// was never built — this is where laziness (and loop termination) lives.
fn swi_rule(net: &mut Net, swi: Port, num: Port) {
    let s = swi.payload() as u32;
    let c = net.cell(s);
    net.free_cell(s);
    let ret = Port(c[0]);
    let s2 = Port(c[1]).payload() as u32;
    let c2 = net.cell(s2);
    net.free_cell(s2);
    let (taken, dead) = if num.as_i64() != 0 { (Port(c2[0]), Port(c2[1])) } else { (Port(c2[1]), Port(c2[0])) };
    link(net, era(), dead);
    net.redexes.push((taken, ret));
}

/// MAT–CON: select the arm whose ctor tag matches the scrutinee, prepend
/// the constructor's fields to the arm closure's captured args and fire
/// it; erase the other arms. `MatchMeta::Proj` is the tuple-projection
/// special case (a 1-way match).
fn mat_rule(net: &mut Net, prog: &NetProg, mat: Port, con: Port) {
    let m = mat_addr(mat);
    let mid = mat_id(mat) as usize;
    let c = net.cell(m);
    net.free_cell(m);
    let ret = Port(c[0]);
    match &prog.metas[mid] {
        MatchMeta::Proj(i) => {
            assert_eq!(con.con_tag(), CTAG_TUPLE, "ICE: projection on non-tuple constructor {}", con.con_tag());
            let fields = con_collect(net, con);
            assert!(*i < fields.len(), "ICE: projection index {} out of bounds ({})", i, fields.len());
            for (j, f) in fields.into_iter().enumerate() {
                if j == *i {
                    link(net, f, ret);
                } else {
                    link(net, era(), f);
                }
            }
        }
        MatchMeta::Arms(tags) => {
            let refs = list_collect(net, Port(c[1]));
            debug_assert_eq!(refs.len(), tags.len());
            let ct = con.con_tag();
            let j = tags
                .iter()
                .position(|t| *t == ct)
                .unwrap_or_else(|| panic!("ICE: match has no arm for ctor tag {} (non-exhaustive at net level)", ct));
            for (i, r) in refs.iter().enumerate() {
                if i != j {
                    link(net, era(), *r);
                }
            }
            let rj = refs[j];
            debug_assert_eq!(rj.tag(), Tag::Ref);
            let mut args = con_collect(net, con); // ctor fields = pattern binders
            args.extend(list_collect(net, ref_head(rj))); // then captured frees
            let head = crate::list_alloc(net, &args);
            net.redexes.push((crate::ref_port(head, ref_entry(rj)), ret));
        }
    }
}

/// DUP–NUM/FLO/CON/LAM: copy. NUM is free to copy; FLO reboxes; CON copies
/// one chain of cells and pushes DUPs onto its fields (lazy recursion);
/// LAM is HVM-style (two lams, body dup, params joined by a same-label dup
/// acting as the superposition) — unreachable from first-order Core v1 and
/// kept only for net-level completeness.
fn dup_rule(net: &mut Net, dup: Port, val: Port) {
    let d = dup_addr(dup);
    let c = net.cell(d);
    net.free_cell(d);
    let (o1, o2) = (Port(c[0]), Port(c[1]));
    match val.tag() {
        Tag::Num => {
            link(net, val, o1);
            link(net, val, o2);
        }
        Tag::Flo => {
            let bits = flo_bits(net.cell(val.payload() as u32));
            let copy = flo_alloc(net, f64::from_bits(bits));
            link(net, val, o1);
            link(net, copy, o2);
        }
        Tag::Con => {
            let ctag = val.con_tag();
            let fields = con_collect(net, val);
            let mut fa = Vec::with_capacity(fields.len());
            let mut fb = Vec::with_capacity(fields.len());
            for f in fields {
                let w1 = wire(net);
                let w2 = wire(net);
                let df = net.alloc(w1, w2);
                link(net, dup_port(df), f);
                fa.push(w1);
                fb.push(w2);
            }
            let ca = con_alloc(net, ctag, &fa);
            let cb = con_alloc(net, ctag, &fb);
            link(net, ca, o1);
            link(net, cb, o2);
        }
        Tag::Lam => {
            let l = val.payload() as u32;
            let cl = net.cell(l);
            net.free_cell(l);
            let (wp1, wp2, wb1, wb2) = (wire(net), wire(net), wire(net), wire(net));
            let l1 = net.alloc(wp1, wb1);
            let l2 = net.alloc(wp2, wb2);
            let db = net.alloc(wb1, wb2);
            link(net, dup_port(db), Port(cl[1])); // copy the body
            let su = net.alloc(wp1, wp2);
            link(net, dup_port(su), Port(cl[0])); // param superposition
            link(net, Port::new(Tag::Lam, l1 as u64), o1);
            link(net, Port::new(Tag::Lam, l2 as u64), o2);
        }
        t => panic!("ICE: no interaction rule for Dup–{:?}", t),
    }
}

/// DUP–DUP with the same label: annihilate (wires cross-connect). v1 has a
/// single label class, so differing labels are an ICE, not a commute.
fn dup_dup(net: &mut Net, a: Port, b: Port) {
    if dup_label(a) != dup_label(b) {
        panic!("ICE: Dup–Dup with distinct labels {}/{} (no commute rule in v1)", dup_label(a), dup_label(b));
    }
    let (ia, ib) = (dup_addr(a), dup_addr(b));
    let ca = net.cell(ia);
    let cb = net.cell(ib);
    net.free_cell(ia);
    net.free_cell(ib);
    link(net, Port(ca[0]), Port(cb[0]));
    link(net, Port(ca[1]), Port(cb[1]));
}
