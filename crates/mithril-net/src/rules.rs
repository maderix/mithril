//! The compile-time program behind the shared rule table
//! (`mithril_core::rules`): what a `Ref` does under the specialization
//! policy, how builtins compute (or stay opaque), and where an op that
//! cannot compute goes (the residual program).

use crate::build::instantiate;
use crate::{list_collect, op_port, ref_entry, ref_head, MatchMeta, Mode, NetProg, ARR_PAIR, PRIM_BASE};
use mithril_core::net::Net;
use mithril_core::port::{Port, Tag};
use mithril_core::rules::{MatMeta, Prog};

pub use mithril_core::rules::{link, process, resolve};

const MASK56: u64 = (1u64 << 56) - 1;

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
        instantiate(net, self, entry, args, other);
        1
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
        let _ = op_port;
        net.residual.push((op, y));
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
/// ints, f64 floats boxed in cells, comparisons producing 0/1). `None`
/// where the op cannot fold at compile time: a failing evaluation (division
/// by zero stays a runtime error), an array builtin (arrays are runtime
/// values), or the pairing pseudo-op.
fn compute(net: &mut Net, code: u16, x: Port, y: Port) -> Option<Port> {
    if code >= PRIM_BASE {
        if code == ARR_PAIR {
            return None;
        }
        let (p, unary) = crate::prim_of_code(code);
        if !p.is_f32() || x.tag() != Tag::Num || (!unary && y.tag() != Tag::Num) {
            return None;
        }
        let args: Vec<i64> = if unary { vec![x.as_i64()] } else { vec![x.as_i64(), y.as_i64()] };
        return Some(Port::num(mithril_front::core::f32_prim(p, &args)));
    }
    match (x.tag(), y.tag()) {
        (Tag::Num, Tag::Num) => {
            let (a, b) = (x.as_i64(), y.as_i64());
            if code >= 16 {
                return Some(cmp_result(code, a.cmp(&b)));
            }
            if matches!(code, 3 | 4 | 5) && b == 0 {
                return None;
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
            Some(Port::num(wrap56(r)))
        }
        (Tag::Flo, Tag::Flo) => {
            let (ax, ay) = (x.payload() as u32, y.payload() as u32);
            let a = f64::from_bits(flo_bits(net.cell(ax)));
            let b = f64::from_bits(flo_bits(net.cell(ay)));
            net.free_cell(ax);
            net.free_cell(ay);
            if code >= 16 {
                let ord = a.partial_cmp(&b).expect("ICE: incomparable floats (NaN)");
                return Some(cmp_result(code, ord));
            }
            let r = match code {
                0 => a + b,
                1 => a - b,
                2 => a * b,
                3 => a / b,
                _ => panic!("ICE: opcode {} not defined on floats", code),
            };
            Some(flo_alloc(net, r))
        }
        (tx, ty) => panic!("ICE: Op fold on mixed operand tags {:?}/{:?}", tx, ty),
    }
}

