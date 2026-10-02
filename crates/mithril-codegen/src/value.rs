//! Value construction shared by native and port lowering. Ownership decisions
//! stay with the caller; these builders preserve field and statement order.
use crate::lir::{as_i, c, let_, num, p, u16_, usize_, v, Pat, Ty, E, S};

pub(crate) fn bind(name: impl Into<String>, ty: Ty, value: E, out: &mut Vec<S>) -> E {
    let name = name.into();
    out.push(let_(&name, ty, value));
    v(name)
}

/// Turn a native integer into its port representation.
pub(crate) fn int_port(value: E, shifted: bool) -> E {
    if shifted { p("retag", vec![value]) } else { num(value) }
}

/// Read a port integer in the native representation selected by its caller.
pub(crate) fn native_int(value: E, shifted: bool) -> E {
    if shifted { p("sh", vec![value]) } else { as_i(value) }
}

/// Read borrowed fields or move them out of an owned constructor. A reuse token retains the
/// two-field cell; ordinary consumption releases its spine. The one-field
/// helper also returns a vacant field, which is released immediately.
/// Native nullary matches skip this operation; port matches consume a chain
/// with zero fields, so the caller chooses whether to invoke it there.
pub(crate) fn fields(src: E, cid: u32, names: Vec<String>, owned: bool, token: Option<String>, out: &mut Vec<S>) -> Vec<E> {
    if !owned {
        assert!(token.is_none(), "borrowed constructor cannot retain a reuse token");
        return (0..names.len()).map(|i| c("field", vec![src.clone(), usize_(i)])).collect();
    }
    let tag = u16_(cid as u64);
    let one = token.is_none() && names.len() == 1;
    let values = names.iter().map(v).collect();
    let (pat, helper) = match (token, names.len()) {
        (Some(token), 2) => {
            let mut fields = names;
            fields.push(token);
            (Pat::Tup(fields), "consume2r".to_string())
        }
        (Some(_), _) => unreachable!("reuse token requires a two-field constructor"),
        (None, 1) => (Pat::Tup(vec![names[0].clone(), "m_unused".into()]), "consume2k".to_string()),
        (None, 2) => (Pat::Tup(names), "consume2k".to_string()),
        (None, n) => (Pat::Arr(names), format!("consume_chain::<{n}>")),
    };
    out.push(S::Let(pat, Ty::Infer, c(&helper, vec![src, tag])));
    if one {
        out.push(crate::lir::free(v("m_unused")));
    }
    values
}
