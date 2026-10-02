//! Contracts for the helper vocabulary used by region proofs. Unknown helpers
//! are barriers; these describe implementations, never define program meaning.
use super::{Bop, Ty, E};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect { Value, Borrow, Write, Own, Allocate, Fuel, Frame }
#[derive(Clone, Copy, Debug)]
pub struct Helper { pub result: Ty, pub effect: Effect, pub ctx: bool, pub may_fail: bool }
pub fn helper(name: &str) -> Option<Helper> {
    use Effect::*;
    let (result, effect, ctx, may_fail) = match name {
        "wrap56" | "as_i" | "sh" => (Ty::I64, Value, false, false),
        "tag" | "num" | "retag" => (Ty::U64, Value, false, false),
        "con_tag" => (Ty::U16, Value, false, false),
        "floor_div" | "py_mod" => (Ty::I64, Value, false, true),
        "field" => (Ty::U64, Borrow, true, true),
        "read_pair" => (Ty::Arr(2), Borrow, true, true),
        "cell_set" => (Ty::Unit, Write, true, true),
        "arr_set_u" => (Ty::U64, Write, false, true),
        "free_val" => (Ty::Unit, Own, true, true),
        "dup_val" => (Ty::U64, Own, true, true),
        "alloc2" | "alloc_rec" => (Ty::U32, Allocate, true, true),
        "work_fuel" | "native_work_fuel" => (Ty::Unit, Fuel, false, false),
        "native_record_read" => (Ty::Infer, Frame, false, true),
        "native_reserve" | "native_take" => (Ty::U64, Frame, false, true),
        "native_record_empty" | "native_empty" | "native_cached" => (Ty::Bool, Frame, false, false),
        "native_get32" | "native_get_fixed32" => (Ty::U32, Frame, false, true),
        "native_get64" | "native_get_fixed64" => (Ty::U64, Frame, false, true),
        "native_record_push" | "native_record_replace" | "native_record_pop" | "native_record_done" | "native_release" | "native_done" | "native_set32" | "native_set64"
            | "native_set_fixed32" | "native_set_fixed64" => (Ty::Unit, Frame, false, true),
        _ => return None,
    };
    Some(Helper { result, effect, ctx, may_fail })
}

/// Separate proof requirements over the same expression walk. A borrowed read
/// is admissible in a copy path, but never in an allocation-free leaf shortcut.
#[derive(Clone, Copy, Debug)]
pub enum Policy { Copy, Exit, Prefix }
impl Policy {
    pub fn accepts(self, e: &E) -> bool {
        e.all(&|e| match e {
            E::Call { f, ctx, .. } => helper(f).is_some_and(|h| h.ctx == *ctx && match self {
                Self::Copy => (!h.may_fail && matches!(f.as_str(), "wrap56" | "tag" | "as_i" | "con_tag")) || (h.effect == Effect::Borrow && f == "field"),
                Self::Exit => h.effect == Effect::Value && !h.may_fail,
                Self::Prefix => h.effect == Effect::Value && matches!(f.as_str(), "wrap56" | "floor_div" | "py_mod" | "sh" | "retag" | "num" | "as_i"),
            }),
            E::V(n) => !matches!(self, Self::Prefix) || n != "fuel",
            E::Int(..) | E::Bool(_) | E::Cast(..) | E::Not(_) | E::Neg(_) => true,
            E::Bin(op, ..) => *op != Bop::Div || matches!(self, Self::Prefix),
            E::Tup(_) => matches!(self, Self::Prefix),
            _ => false,
        })
    }
}
