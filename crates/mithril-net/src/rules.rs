//! The compile-time program behind the shared rule table
//! (`mithril_core::rules`): what a `Ref` does under the specialization
//! policy, how builtins compute (or stay opaque), and where an op that
//! cannot compute goes (the residual program).

use crate::{flo_bits, instantiate, list_collect, ref_entry, ref_head, MatchMeta, Mode, NetProg, ARR_PAIR, PRIM_BASE};
use mithril_core::net::Net;
use mithril_front::ast::{BinOp, CmpOp};
use mithril_front::core::{cmp_bool, flo_op, int_op};
use mithril_core::port::{Port, Tag};
use mithril_core::rules::{MatMeta, Prog};

pub use mithril_core::rules::{link, process, resolve};


impl Prog<Net> for NetProg {
    /// REF-unfold: splice the entry's body, except that specialization
    /// keeps a call to a real function as a call unless the policy unfolds
    /// it (lifted branches and arms always unfold: they are bodies).
    fn unfold(&self, net: &mut Net, r: Port, other: Port) -> u64 {
        let entry = ref_entry(r) as usize;
        if self.mode == Mode::Specialize && entry < self.nfns && !self.inline[entry] {
            net.residual.push((r, other));
            return 0;
        }
        let args = list_collect(net, ref_head(r));
        instantiate(net, &self.entries, entry, args, other);
        1
    }

    fn is_closure(&self, r: Port) -> bool {
        ref_entry(r) as usize >= self.nfns
    }

    fn mat_meta(&self, mid: u16) -> MatMeta<'_> {
        match &self.metas[mid as usize] {
            MatchMeta::Proj(i) => MatMeta::Proj(*i),
            MatchMeta::Arms(tags) => MatMeta::Arms(tags),
        }
    }

    fn compute(&self, net: &mut Net, code: u16, x: Port, y: Port) -> Option<Port> {
        compute(net, code, x, y)
    }

    /// Cannot fold at compile time: the op stays in the residual program
    /// as (op with cell [x, ret], y).
    fn park_op(&self, net: &mut Net, op: Port, y: Port) {
        net.residual.push((op, y));
    }
}


fn cmp_result(code: u16, ord: Option<std::cmp::Ordering>) -> Port {
    Port::num(cmp_bool(CmpOp::ALL[(code - 16) as usize], ord) as i64)
}

/// The integer an operand holds; a consumed boxed operand is released.
fn take_int(p: Port) -> i64 {
    let v = p.int_value();
    if p.tag() == Tag::Big {
        p.big_release();
    }
    v
}

/// Numeric fold with `eval_core`'s semantics (`int_op`, `flo_op`,
/// `cmp_bool`; floats boxed in cells, comparisons producing 0/1). `None`
/// where the op cannot fold at compile time: a failing evaluation (division
/// by zero stays a runtime error), an array builtin (arrays are runtime
/// values), or the pairing pseudo-op.
fn compute(net: &mut Net, code: u16, x: Port, y: Port) -> Option<Port> {
    if code >= PRIM_BASE {
        if code == ARR_PAIR {
            return None;
        }
        let (p, unary) = crate::prim_of_code(code);
        let int = |p: Port| matches!(p.tag(), Tag::Num | Tag::Big);
        if !p.is_f32() || !int(x) || (!unary && !int(y)) {
            return None;
        }
        let args: Vec<i64> = if unary { vec![take_int(x)] } else { vec![take_int(x), take_int(y)] };
        return Some(Port::int(mithril_front::core::f32_prim(p, &args)));
    }
    match (x.tag(), y.tag()) {
        (Tag::Num | Tag::Big, Tag::Num | Tag::Big) => {
            let (a, b) = (take_int(x), take_int(y));
            if code >= 16 {
                return Some(cmp_result(code, Some(a.cmp(&b))));
            }
            int_op(BinOp::ALL[code as usize], a, b).map(Port::int)
        }
        (Tag::Flo, Tag::Flo) => {
            let (ax, ay) = (x.payload() as u32, y.payload() as u32);
            let a = f64::from_bits(flo_bits(net.cell(ax)));
            let b = f64::from_bits(flo_bits(net.cell(ay)));
            net.free_cell(ax);
            net.free_cell(ay);
            if code >= 16 {
                return Some(cmp_result(code, a.partial_cmp(&b)));
            }
            let r = flo_op(BinOp::ALL[code as usize], a, b).unwrap_or_else(|| panic!("ICE: opcode {} not defined on floats", code));
            Some(mithril_core::agents::Cells::alloc_flo(net, r))
        }
        (tx, ty) => panic!("type error: arithmetic on mixed int/float operands ({:?}/{:?})", tx, ty),
    }
}

