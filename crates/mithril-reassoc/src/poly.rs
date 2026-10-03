//! Polynomial normal form over Z_2^k (wrapping arithmetic), k = 64 for
//! native int folds and k = 32 for `& 4294967295`-masked (u32-emulation)
//! folds.
//!
//! A `Poly` maps a monomial (a sorted `(variable, exponent)` list) to its
//! coefficient mod 2^k, where `mask = 2^k - 1` is threaded through every
//! operation. Expressions built from wrapping `+ - *` agree on all inputs
//! iff their normal forms are equal — the fact the prover in `detect`
//! relies on. Signedness is irrelevant: two's-complement wrapping
//! `+ - *` are exactly the ring operations of Z_2^k.

use std::collections::{BTreeMap, HashMap};

/// Coefficient mask for native int folds (Z_2^64).
pub const MASK64: u64 = u64::MAX;
/// Coefficient mask for `& 4294967295`-masked folds (Z_2^32).
pub const MASK32: u64 = (1u64 << 32) - 1;

/// A monomial: variables with positive exponents, sorted by name.
pub type Mono = Vec<(String, u32)>;
/// Sparse polynomial: monomial -> nonzero coefficient mod 2^k.
pub type Poly = BTreeMap<Mono, u64>;

pub fn pconst(c: i64, mask: u64) -> Poly {
    pconst_u(c as u64, mask)
}

fn pconst_u(c: u64, mask: u64) -> Poly {
    let mut p = Poly::new();
    if c & mask != 0 {
        p.insert(Vec::new(), c & mask);
    }
    p
}

pub fn pvar(v: &str) -> Poly {
    let mut p = Poly::new();
    p.insert(vec![(v.to_string(), 1)], 1);
    p
}

pub fn padd(p: &Poly, q: &Poly, mask: u64) -> Poly {
    let mut r = p.clone();
    for (m, c) in q {
        let e = r.entry(m.clone()).or_insert(0);
        *e = e.wrapping_add(*c) & mask; // mod 2^64 first: 2^k divides it
        if *e == 0 {
            r.remove(m);
        }
    }
    r
}

pub fn pneg(p: &Poly, mask: u64) -> Poly {
    p.iter().map(|(m, c)| (m.clone(), c.wrapping_neg() & mask)).collect()
}

pub fn pmul(p: &Poly, q: &Poly, mask: u64) -> Poly {
    let mut r = Poly::new();
    for (m1, c1) in p {
        for (m2, c2) in q {
            let mut d: BTreeMap<&String, u32> = BTreeMap::new();
            for (v, e) in m1.iter().chain(m2.iter()) {
                *d.entry(v).or_insert(0) += *e;
            }
            let m: Mono = d.into_iter().map(|(v, e)| (v.clone(), e)).collect();
            let c = ((*c1 as u128 * *c2 as u128) & mask as u128) as u64;
            let e = r.entry(m).or_insert(0);
            *e = e.wrapping_add(c) & mask;
        }
    }
    r.retain(|_, c| *c != 0);
    r
}

/// Substitute polynomials for variables; `env` must bind every variable
/// occurring in `p` (guaranteed by construction in `detect::prove`).
pub fn psubst(p: &Poly, env: &HashMap<String, Poly>, mask: u64) -> Poly {
    let mut out = Poly::new();
    for (m, c) in p {
        let mut term = pconst_u(*c, mask);
        for (v, e) in m {
            let vp = env.get(v).unwrap_or_else(|| panic!("psubst: unbound variable {v}"));
            for _ in 0..*e {
                term = pmul(&term, vp, mask);
            }
        }
        out = padd(&out, &term, mask);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_identities_mod_2_64() {
        // (x + y)^2 == x^2 + 2xy + y^2
        let (x, y) = (pvar("x"), pvar("y"));
        let s = padd(&x, &y, MASK64);
        let lhs = pmul(&s, &s, MASK64);
        let rhs = padd(
            &padd(&pmul(&x, &x, MASK64), &pmul(&pconst(2, MASK64), &pmul(&x, &y, MASK64), MASK64), MASK64),
            &pmul(&y, &y, MASK64),
            MASK64,
        );
        assert_eq!(lhs, rhs);
        // x - x == 0, and coefficients wrap mod 2^64 (sums and products
        // past 2^64 included), not mod 2^56
        assert!(padd(&x, &pneg(&x, MASK64), MASK64).is_empty());
        assert_eq!(pconst(-1, MASK64), pconst_u(u64::MAX, MASK64));
        assert!(!pconst(1i64 << 56, MASK64).is_empty());
        assert!(pmul(&pconst(1 << 32, MASK64), &pconst(1 << 32, MASK64), MASK64).is_empty());
        assert!(padd(&pconst(i64::MIN, MASK64), &pconst(i64::MIN, MASK64), MASK64).is_empty());
        assert_eq!(padd(&pconst(-1, MASK64), &pconst(-1, MASK64), MASK64), pconst(-2, MASK64));
    }

    #[test]
    fn ring_identities_mod_2_32() {
        assert!(pconst(1i64 << 32, MASK32).is_empty()); // 2^32 == 0 (mod 2^32)
        assert_eq!(pconst(-1, MASK32), pconst((1i64 << 32) - 1, MASK32));
        assert!(pmul(&pconst(1 << 16, MASK32), &pconst(1 << 16, MASK32), MASK32).is_empty());
        // but 2^32 != 0 in the 64-bit ring
        assert!(!pconst(1i64 << 32, MASK64).is_empty());
    }
}
