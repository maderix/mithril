//! Polynomial normal form over Z_2^56 (wrapping i56 arithmetic).
//!
//! A `Poly` maps a monomial (a sorted `(variable, exponent)` list) to its
//! coefficient mod 2^56. Expressions built from wrapping `+ - *` agree on
//! all inputs iff their normal forms are equal — the fact the prover in
//! `detect` relies on. Signedness is irrelevant: i56 two's-complement
//! wrapping `+ - *` are exactly the ring operations of Z_2^56.

use std::collections::{BTreeMap, HashMap};

/// Coefficient modulus (2^56).
pub const MOD: u128 = 1 << 56;
const MASK: u64 = (1u64 << 56) - 1;

/// A monomial: variables with positive exponents, sorted by name.
pub type Mono = Vec<(String, u32)>;
/// Sparse polynomial: monomial -> nonzero coefficient mod 2^56.
pub type Poly = BTreeMap<Mono, u64>;

pub fn pconst(c: i64) -> Poly {
    pconst_u((c as u64) & MASK)
}

fn pconst_u(c: u64) -> Poly {
    let mut p = Poly::new();
    if c & MASK != 0 {
        p.insert(Vec::new(), c & MASK);
    }
    p
}

pub fn pvar(v: &str) -> Poly {
    let mut p = Poly::new();
    p.insert(vec![(v.to_string(), 1)], 1);
    p
}

pub fn padd(p: &Poly, q: &Poly) -> Poly {
    let mut r = p.clone();
    for (m, c) in q {
        let e = r.entry(m.clone()).or_insert(0);
        *e = (*e + *c) & MASK; // both < 2^56, no u64 overflow
        if *e == 0 {
            r.remove(m);
        }
    }
    r
}

pub fn pneg(p: &Poly) -> Poly {
    p.iter().map(|(m, c)| (m.clone(), c.wrapping_neg() & MASK)).collect()
}

pub fn pmul(p: &Poly, q: &Poly) -> Poly {
    let mut r = Poly::new();
    for (m1, c1) in p {
        for (m2, c2) in q {
            let mut d: BTreeMap<&String, u32> = BTreeMap::new();
            for (v, e) in m1.iter().chain(m2.iter()) {
                *d.entry(v).or_insert(0) += *e;
            }
            let m: Mono = d.into_iter().map(|(v, e)| (v.clone(), e)).collect();
            let c = ((*c1 as u128 * *c2 as u128) % MOD) as u64;
            let e = r.entry(m).or_insert(0);
            *e = (*e + c) & MASK;
        }
    }
    r.retain(|_, c| *c != 0);
    r
}

/// Substitute polynomials for variables; `env` must bind every variable
/// occurring in `p` (guaranteed by construction in `detect::prove`).
pub fn psubst(p: &Poly, env: &HashMap<String, Poly>) -> Poly {
    let mut out = Poly::new();
    for (m, c) in p {
        let mut term = pconst_u(*c);
        for (v, e) in m {
            let vp = env.get(v).unwrap_or_else(|| panic!("psubst: unbound variable {v}"));
            for _ in 0..*e {
                term = pmul(&term, vp);
            }
        }
        out = padd(&out, &term);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_identities_mod_2_56() {
        // (x + y)^2 == x^2 + 2xy + y^2
        let (x, y) = (pvar("x"), pvar("y"));
        let s = padd(&x, &y);
        let lhs = pmul(&s, &s);
        let rhs = padd(
            &padd(&pmul(&x, &x), &pmul(&pconst(2), &pmul(&x, &y))),
            &pmul(&y, &y),
        );
        assert_eq!(lhs, rhs);
        // x - x == 0, and coefficients wrap mod 2^56
        assert!(padd(&x, &pneg(&x)).is_empty());
        assert!(pconst(1i64 << 56).is_empty()); // 2^56 == 0 (mod 2^56)
        assert_eq!(pconst(-1), pconst((1i64 << 56) - 1));
        assert!(pmul(&pconst(1 << 28), &pconst(1 << 28)).is_empty()); // 2^56 == 0
    }
}
