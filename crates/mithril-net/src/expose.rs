//! Readback order that exposes forks.
//!
//! The net has no order between independent redexes; readback chooses one. A
//! call read back before a test that does not depend on it makes everything
//! after the call wait for it, even when one arm of the test holds a second,
//! independent call:
//!
//! ```text
//! x = f(a); y = g(..); if c: A else B        (c, g do not read x; B calls f(b))
//! ```
//!
//! reads back as
//!
//! ```text
//! y = g(..); if c: (x = f(a); A) else (x' = f(a); B)
//! ```
//!
//! Each path still evaluates `f(a)` once where its arm uses it; an arm that does
//! not use it gets no binding, as the net erases a call whose result is unused.
//! In `B` the two calls are now
//! independent bindings that the frame split runs in parallel. The rewrite is
//! applied only where it exposes such a call.

use mithril_front::core::Core;

/// Read `body` back with calls sunk past the tests that do not depend on them.
pub fn expose_forks(body: Core) -> Core {
    let mut next = body.max_var() + 1;
    sink(body, &mut next)
}

fn sink(e: Core, next: &mut u32) -> Core {
    let e = map_kids(e, &mut |k| sink(k, next));
    let Core::Let(x, r, rest) = e else { return e };
    if !matches!(*r, Core::Call(..)) {
        return Core::Let(x, r, rest);
    }
    // the lets between the call and the test, none reading `x`
    let mut prefix = Vec::new();
    let mut cur = *rest;
    while let Core::Let(y, ry, b) = cur {
        if ry.reads(x) {
            cur = Core::Let(y, ry, b);
            break;
        }
        prefix.push((y, *ry));
        cur = *b;
    }
    let Core::If(c, a, b) = cur else {
        return Core::Let(x, r, Box::new(rebuild(prefix, cur)));
    };
    let exposes = |arm: &Core| arm.any(&mut |n| matches!(n, Core::Call(_, args) if !args.iter().any(|a| a.reads(x))).then_some(true));
    if c.reads(x) || !(exposes(&a) || exposes(&b)) {
        return Core::Let(x, r, Box::new(rebuild(prefix, Core::If(c, a, b))));
    }
    let a = if a.reads(x) { Box::new(Core::Let(x, r.clone(), a)) } else { a };
    let b = if b.reads(x) {
        let x2 = *next;
        *next += 1;
        Box::new(Core::Let(x2, r, Box::new(b.rename(&mut |v| if v == x { x2 } else { v }))))
    } else {
        b
    };
    rebuild(prefix, Core::If(c, a, b))
}

fn rebuild(prefix: Vec<(u32, Core)>, tail: Core) -> Core {
    prefix.into_iter().rev().fold(tail, |acc, (y, ry)| Core::Let(y, Box::new(ry), Box::new(acc)))
}

/// Apply `f` to each immediate subexpression, keeping binders.
pub(crate) fn map_kids(e: Core, f: &mut dyn FnMut(Core) -> Core) -> Core {
    let mut b = |x: Box<Core>| Box::new(f(*x));
    match e {
        Core::Op2(op, x, y) => Core::Op2(op, b(x), b(y)),
        Core::Cmp(op, x, y) => Core::Cmp(op, b(x), b(y)),
        Core::If(c, t, e) => Core::If(b(c), b(t), b(e)),
        Core::Let(x, r, body) => Core::Let(x, b(r), b(body)),
        Core::Call(g, args) => Core::Call(g, args.into_iter().map(&mut *f).collect()),
        Core::Ctor(c, args) => Core::Ctor(c, args.into_iter().map(&mut *f).collect()),
        Core::Match(s, arms) => Core::Match(b(s), arms.into_iter().map(|(c, bs, body)| (c, bs, f(body))).collect()),
        Core::Reuse(v, c, args) => Core::Reuse(v, c, args.into_iter().map(&mut *f).collect()),
        Core::Tuple(xs) => Core::Tuple(xs.into_iter().map(&mut *f).collect()),
        Core::Proj(x, i) => Core::Proj(b(x), i),
        Core::Prim(p, xs) => Core::Prim(p, xs.into_iter().map(&mut *f).collect()),
        Core::Lam(x, body) => Core::Lam(x, b(body)),
        Core::App(g, a) => Core::App(b(g), b(a)),
        leaf => leaf,
    }
}
