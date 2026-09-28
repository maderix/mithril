//! Static Core → Core rewrites that run before emission. Each is a
//! semantics-preserving transformation on the IR (checkable against
//! `eval_core`); the emitter only materializes what they decided.
//!
//! * `inline_leaves`: a call to a call-free, non-recursive function whose
//!   body is small is replaced by that body with the arguments let-bound.
//! * `mark_reuse`: a constructor built on a call-free straight-line path
//!   after a match consumed a same-arity constructor cell is rewritten to
//!   `Reuse(v, c, args)`: build in the dead cell instead of allocating. The
//!   token never crosses a call, a value-position branch, or a loop
//!   back-edge (the free-early rule: the cell returns to the LIFO free list
//!   before other work runs), and every path that does not reuse a token
//!   releases it at its terminal (the emitter emits that release).

use crate::{has_call, max_var};
use mithril_front::core::{Core, CoreModule, UNREACHABLE_CTOR};
use std::collections::HashMap;

/// Node count of an expression (size heuristic for inlining).
fn size(e: &Core) -> usize {
    match e {
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => 1,
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => 1 + size(a) + size(b),
        Core::If(c, t, f) => 1 + size(c) + size(t) + size(f),
        Core::Let(_, r, b) => 1 + size(r) + size(b),
        Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) => {
            1 + xs.iter().map(size).sum::<usize>()
        }
        Core::Proj(b, _) => 1 + size(b),
        Core::Match(s, arms) => 1 + size(s) + arms.iter().map(|(_, _, b)| size(b)).sum::<usize>(),
    }
}

fn subst(e: &Core, map: &HashMap<u32, u32>) -> Core {
    match e {
        Core::Var(i) => Core::Var(*map.get(i).unwrap_or(i)),
        Core::Num(_) | Core::Flo(_) => e.clone(),
        Core::Op2(o, a, b) => Core::Op2(o.clone(), Box::new(subst(a, map)), Box::new(subst(b, map))),
        Core::Cmp(o, a, b) => Core::Cmp(o.clone(), Box::new(subst(a, map)), Box::new(subst(b, map))),
        Core::If(c, t, f) => Core::If(Box::new(subst(c, map)), Box::new(subst(t, map)), Box::new(subst(f, map))),
        Core::Let(x, r, b) => Core::Let(*x, Box::new(subst(r, map)), Box::new(subst(b, map))),
        Core::Call(g, xs) => Core::Call(*g, xs.iter().map(|x| subst(x, map)).collect()),
        Core::Ctor(c, xs) => Core::Ctor(*c, xs.iter().map(|x| subst(x, map)).collect()),
        Core::Reuse(v, c, xs) => Core::Reuse(*map.get(v).unwrap_or(v), *c, xs.iter().map(|x| subst(x, map)).collect()),
        Core::Tuple(xs) => Core::Tuple(xs.iter().map(|x| subst(x, map)).collect()),
        Core::Proj(b, i) => Core::Proj(Box::new(subst(b, map)), *i),
        Core::Match(s, arms) => Core::Match(
            Box::new(subst(s, map)),
            arms.iter().map(|(c, bs, b)| (*c, bs.clone(), subst(b, map))).collect(),
        ),
    }
}

/// Inline small call-free functions at their call sites (module-wide).
pub(crate) fn inline_leaves(m: &CoreModule) -> CoreModule {
    const MAX_SIZE: usize = 96;
    let leafy: Vec<bool> = m
        .fns
        .iter()
        .enumerate()
        .map(|(i, f)| i as u32 != m.main && !has_call(&f.body) && size(&f.body) <= MAX_SIZE)
        .collect();
    let mut out = m.clone();
    for (fid, f) in out.fns.iter_mut().enumerate() {
        if leafy[fid] {
            continue; // leaves have no calls to inline
        }
        let mut next = max_var(&f.body).max(f.arity as u32) + 1;
        f.body = inline_in(&f.body, m, &leafy, &mut next);
    }
    out
}

fn inline_in(e: &Core, m: &CoreModule, leafy: &[bool], next: &mut u32) -> Core {
    let rec = |x: &Core, next: &mut u32| inline_in(x, m, leafy, next);
    match e {
        Core::Call(g, args) if leafy[*g as usize] => {
            let callee = &m.fns[*g as usize];
            // fresh binders for the parameters; the callee body is closed so
            // its own binders cannot capture caller variables
            let base = *next;
            *next += callee.arity as u32;
            let shift = *next;
            *next += max_var(&callee.body).max(callee.arity as u32) + 1;
            let mut map = HashMap::new();
            for p in 0..callee.arity as u32 {
                map.insert(p, base + p);
            }
            // rename the callee's own binders past the caller's range too
            let body = shift_binders(&callee.body, shift, callee.arity as u32, &map);
            let mut out = body;
            for (p, a) in args.iter().enumerate().rev() {
                out = Core::Let(base + p as u32, Box::new(rec(a, next)), Box::new(out));
            }
            out
        }
        Core::Call(g, args) => Core::Call(*g, args.iter().map(|a| rec(a, next)).collect()),
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => e.clone(),
        Core::Op2(o, a, b) => Core::Op2(o.clone(), Box::new(rec(a, next)), Box::new(rec(b, next))),
        Core::Cmp(o, a, b) => Core::Cmp(o.clone(), Box::new(rec(a, next)), Box::new(rec(b, next))),
        Core::If(c, t, f) => Core::If(Box::new(rec(c, next)), Box::new(rec(t, next)), Box::new(rec(f, next))),
        Core::Let(x, r, b) => Core::Let(*x, Box::new(rec(r, next)), Box::new(rec(b, next))),
        Core::Ctor(c, xs) => Core::Ctor(*c, xs.iter().map(|x| rec(x, next)).collect()),
        Core::Reuse(v, c, xs) => Core::Reuse(*v, *c, xs.iter().map(|x| rec(x, next)).collect()),
        Core::Tuple(xs) => Core::Tuple(xs.iter().map(|x| rec(x, next)).collect()),
        Core::Proj(b, i) => Core::Proj(Box::new(rec(b, next)), *i),
        Core::Match(s, arms) => Core::Match(
            Box::new(rec(s, next)),
            arms.iter().map(|(c, bs, b)| (*c, bs.clone(), rec(b, next))).collect(),
        ),
    }
}

/// Rename every binder of a closed body: params via `map`, locals to
/// `shift + old`.
fn shift_binders(e: &Core, shift: u32, arity: u32, map: &HashMap<u32, u32>) -> Core {
    let mv = |i: u32| if i < arity { map[&i] } else { shift + i };
    match e {
        Core::Var(i) => Core::Var(mv(*i)),
        Core::Num(_) | Core::Flo(_) => e.clone(),
        Core::Op2(o, a, b) => Core::Op2(o.clone(), Box::new(shift_binders(a, shift, arity, map)), Box::new(shift_binders(b, shift, arity, map))),
        Core::Cmp(o, a, b) => Core::Cmp(o.clone(), Box::new(shift_binders(a, shift, arity, map)), Box::new(shift_binders(b, shift, arity, map))),
        Core::If(c, t, f) => Core::If(Box::new(shift_binders(c, shift, arity, map)), Box::new(shift_binders(t, shift, arity, map)), Box::new(shift_binders(f, shift, arity, map))),
        Core::Let(x, r, b) => Core::Let(mv(*x), Box::new(shift_binders(r, shift, arity, map)), Box::new(shift_binders(b, shift, arity, map))),
        Core::Call(g, xs) => Core::Call(*g, xs.iter().map(|x| shift_binders(x, shift, arity, map)).collect()),
        Core::Ctor(c, xs) => Core::Ctor(*c, xs.iter().map(|x| shift_binders(x, shift, arity, map)).collect()),
        Core::Reuse(v, c, xs) => Core::Reuse(mv(*v), *c, xs.iter().map(|x| shift_binders(x, shift, arity, map)).collect()),
        Core::Tuple(xs) => Core::Tuple(xs.iter().map(|x| shift_binders(x, shift, arity, map)).collect()),
        Core::Proj(b, i) => Core::Proj(Box::new(shift_binders(b, shift, arity, map)), *i),
        Core::Match(s, arms) => Core::Match(
            Box::new(shift_binders(s, shift, arity, map)),
            arms.iter()
                .map(|(c, bs, b)| (*c, bs.iter().map(|x| mv(*x)).collect(), shift_binders(b, shift, arity, map)))
                .collect(),
        ),
    }
}

// ---------------------------------------------------------------- reuse

fn count_uses(e: &Core, m: &mut HashMap<u32, u32>) {
    match e {
        Core::Var(i) => *m.entry(*i).or_insert(0) += 1,
        Core::Num(_) | Core::Flo(_) => {}
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
            count_uses(a, m);
            count_uses(b, m);
        }
        Core::If(c, t, f) => {
            count_uses(c, m);
            count_uses(t, m);
            count_uses(f, m);
        }
        Core::Let(_, r, b) => {
            count_uses(r, m);
            count_uses(b, m);
        }
        Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) => xs.iter().for_each(|x| count_uses(x, m)),
        Core::Reuse(v, _, xs) => {
            *m.entry(*v).or_insert(0) += 1;
            xs.iter().for_each(|x| count_uses(x, m));
        }
        Core::Proj(b, _) => count_uses(b, m),
        Core::Match(s, arms) => {
            count_uses(s, m);
            arms.iter().for_each(|(_, _, b)| count_uses(b, m));
        }
    }
}

pub(crate) struct ReuseCtx<'a> {
    pub m: &'a CoreModule,
    pub unbox: &'a HashMap<u32, u8>,
    pub uses: HashMap<u32, u32>,
}

/// Mark reuse on a normalized, uniquified body (tail position entry).
pub(crate) fn mark_reuse(body: &Core, m: &CoreModule, unbox: &HashMap<u32, u8>) -> Core {
    let mut uses = HashMap::new();
    count_uses(body, &mut uses);
    let cx = ReuseCtx { m, unbox, uses };
    let mut avail: Vec<(u32, usize)> = Vec::new();
    tail(body, &cx, &mut avail)
}

fn take_token(avail: &mut Vec<(u32, usize)>, arity: usize) -> Option<u32> {
    let pos = avail.iter().rposition(|(_, a)| *a == arity)?;
    Some(avail.remove(pos).0)
}

/// Value position: a construct may take a token; calls and branches end
/// the straight-line span (branches evaluate with no tokens inside).
fn val(e: &Core, cx: &ReuseCtx, avail: &mut Vec<(u32, usize)>) -> Core {
    match e {
        Core::Ctor(c, xs) => {
            let xs2: Vec<Core> = xs.iter().map(|x| val(x, cx, avail)).collect();
            let ar = cx.m.ctors[*c as usize].1;
            if ar == 2 && !cx.unbox.contains_key(c) {
                if let Some(v) = take_token(avail, 2) {
                    return Core::Reuse(v, *c, xs2);
                }
            }
            Core::Ctor(*c, xs2)
        }
        Core::Call(g, xs) => {
            let xs2 = xs.iter().map(|x| val(x, cx, avail)).collect();
            avail.clear();
            Core::Call(*g, xs2)
        }
        Core::If(c, t, f) => {
            let c2 = val(c, cx, avail);
            let mut none = Vec::new();
            let t2 = val(t, cx, &mut none);
            none.clear();
            let f2 = val(f, cx, &mut none);
            avail.clear();
            Core::If(Box::new(c2), Box::new(t2), Box::new(f2))
        }
        Core::Match(s, arms) => {
            let s2 = val(s, cx, avail);
            let mut none = Vec::new();
            let arms2 = arms
                .iter()
                .map(|(c, bs, b)| {
                    none.clear();
                    (*c, bs.clone(), val(b, cx, &mut none))
                })
                .collect();
            avail.clear();
            Core::Match(Box::new(s2), arms2)
        }
        Core::Let(x, r, b) => {
            let r2 = val(r, cx, avail);
            let b2 = val(b, cx, avail);
            Core::Let(*x, Box::new(r2), Box::new(b2))
        }
        Core::Op2(o, a, b) => Core::Op2(o.clone(), Box::new(val(a, cx, avail)), Box::new(val(b, cx, avail))),
        Core::Cmp(o, a, b) => Core::Cmp(o.clone(), Box::new(val(a, cx, avail)), Box::new(val(b, cx, avail))),
        Core::Tuple(xs) => Core::Tuple(xs.iter().map(|x| val(x, cx, avail)).collect()),
        Core::Proj(b, i) => Core::Proj(Box::new(val(b, cx, avail)), *i),
        Core::Reuse(v, c, xs) => Core::Reuse(*v, *c, xs.iter().map(|x| val(x, cx, avail)).collect()),
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => e.clone(),
    }
}

/// Tail position: branches inherit the tokens (each path independently).
fn tail(e: &Core, cx: &ReuseCtx, avail: &mut Vec<(u32, usize)>) -> Core {
    match e {
        Core::Let(x, r, b) => {
            let r2 = val(r, cx, avail);
            let b2 = tail(b, cx, avail);
            Core::Let(*x, Box::new(r2), Box::new(b2))
        }
        Core::If(c, t, f) => {
            let c2 = val(c, cx, avail);
            let mut a1 = avail.clone();
            let t2 = tail(t, cx, &mut a1);
            let mut a2 = avail.clone();
            let f2 = tail(f, cx, &mut a2);
            avail.clear();
            Core::If(Box::new(c2), Box::new(t2), Box::new(f2))
        }
        Core::Match(s, arms) => {
            let s2 = val(s, cx, avail);
            // a consumed boxed scrutinee cell becomes a token for each arm
            let consumed = match &**s {
                Core::Var(v) => cx.uses.get(v).copied().unwrap_or(0) == 1,
                _ => true,
            };
            let arms2 = arms
                .iter()
                .map(|(c, bs, b)| {
                    let mut a = avail.clone();
                    if consumed && *c != UNREACHABLE_CTOR && !cx.unbox.contains_key(c) {
                        if let Core::Var(v) = &**s {
                            let ar = cx.m.ctors[*c as usize].1;
                            if ar == 2 {
                                a.push((*v, ar));
                            }
                        }
                    }
                    (*c, bs.clone(), tail(b, cx, &mut a))
                })
                .collect();
            avail.clear();
            Core::Match(Box::new(s2), arms2)
        }
        other => val(other, cx, avail),
    }
}

// ------------------------------------------------- record-to-tuple rewrite

/// A datatype with exactly one constructor whose fields are all Int is a
/// plain record: represent it as a tuple so the scalar lowering carries it
/// in native registers (`Ctor(c, xs)` -> `Tuple(xs)`, a single-arm match
/// -> projections). Skipped for the type of `main`'s result (printing
/// would change) and for any class that also has other constructors.
pub(crate) fn records_to_tuples(m: &CoreModule, tys: &crate::ty::Types) -> CoreModule {
    use crate::ty::Ty;
    let n = m.ctors.len();
    let mut per_class: HashMap<u32, Vec<u32>> = HashMap::new();
    for c in 0..n as u32 {
        per_class.entry(tys.class_of[c as usize]).or_default().push(c);
    }
    let main_ret = tys.ret[m.main as usize];
    let mut rec: Vec<bool> = vec![false; n];
    for (class, cs) in &per_class {
        if cs.len() != 1 {
            continue;
        }
        let c = cs[0] as usize;
        let ar = m.ctors[c].1;
        if ar == 0 || ar > 8 {
            continue;
        }
        if !tys.field[c].iter().all(|t| *t == Ty::Int) {
            continue;
        }
        if main_ret == Ty::Adt(*class) {
            continue;
        }
        rec[c] = true;
    }
    if !rec.iter().any(|b| *b) {
        return m.clone();
    }
    fn go(e: &Core, rec: &[bool], next: &mut u32) -> Core {
        match e {
            Core::Ctor(c, xs) if rec[*c as usize] => Core::Tuple(xs.iter().map(|x| go(x, rec, next)).collect()),
            Core::Reuse(_, c, xs) if rec[*c as usize] => Core::Tuple(xs.iter().map(|x| go(x, rec, next)).collect()),
            Core::Match(s, arms) if arms.len() == 1 && rec[arms[0].0 as usize] => {
                let (_, bs, body) = &arms[0];
                let t = *next;
                *next += 1;
                let mut out = go(body, rec, next);
                for (i, b) in bs.iter().enumerate().rev() {
                    out = Core::Let(*b, Box::new(Core::Proj(Box::new(Core::Var(t)), i)), Box::new(out));
                }
                Core::Let(t, Box::new(go(s, rec, next)), Box::new(out))
            }
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => e.clone(),
            Core::Op2(o, a, b) => Core::Op2(o.clone(), Box::new(go(a, rec, next)), Box::new(go(b, rec, next))),
            Core::Cmp(o, a, b) => Core::Cmp(o.clone(), Box::new(go(a, rec, next)), Box::new(go(b, rec, next))),
            Core::If(c, t, f) => Core::If(Box::new(go(c, rec, next)), Box::new(go(t, rec, next)), Box::new(go(f, rec, next))),
            Core::Let(x, r, b) => Core::Let(*x, Box::new(go(r, rec, next)), Box::new(go(b, rec, next))),
            Core::Call(g, xs) => Core::Call(*g, xs.iter().map(|x| go(x, rec, next)).collect()),
            Core::Ctor(c, xs) => Core::Ctor(*c, xs.iter().map(|x| go(x, rec, next)).collect()),
            Core::Reuse(v, c, xs) => Core::Reuse(*v, *c, xs.iter().map(|x| go(x, rec, next)).collect()),
            Core::Tuple(xs) => Core::Tuple(xs.iter().map(|x| go(x, rec, next)).collect()),
            Core::Proj(b, i) => Core::Proj(Box::new(go(b, rec, next)), *i),
            Core::Match(s, arms) => Core::Match(
                Box::new(go(s, rec, next)),
                arms.iter().map(|(c, bs, b)| (*c, bs.clone(), go(b, rec, next))).collect(),
            ),
        }
    }
    let mut out = m.clone();
    for f in out.fns.iter_mut() {
        let mut next = max_var(&f.body).max(f.arity as u32) + 1;
        f.body = go(&f.body, &rec, &mut next);
    }
    out
}


// ---- compile-time unfolding of calls with static control ----
//
// A call whose callee's control flow is decided entirely by its constant
// arguments reduces at compile time to straight-line residual code: the
// statically reducible part of the program is reduced before it runs (the
// dynamic values stay as let-bound variables). Constant operations fold
// with the reference interpreter's i56 semantics. Anything dynamic in
// control position (a branch on a runtime value, a match, a constructor)
// or over budget leaves the call as it was.

/// Calls unfolded per site, residual bindings per site, and evaluation
/// steps per site (constant work is free at runtime but not at compile time).
const UNFOLD_CALLS: usize = 256;
const UNFOLD_SIZE: usize = 2000;
const UNFOLD_WORK: usize = 200_000;

#[derive(Clone)]
enum Pv {
    K(i64),
    D(Core), // an atom: Num or Var of the caller
}

impl Pv {
    fn core(&self) -> Core {
        match self {
            Pv::K(n) => Core::Num(*n),
            Pv::D(c) => c.clone(),
        }
    }
}

fn wrap56(v: i64) -> i64 {
    ((v as u64) << 8) as i64 >> 8
}

/// Fold an int op exactly as `eval_core` does; None where evaluation would
/// fail at runtime (division by zero), so the failure stays at runtime.
fn fold_op2(op: &mithril_front::ast::BinOp, x: i64, y: i64) -> Option<i64> {
    use mithril_front::ast::BinOp::*;
    Some(wrap56(match op {
        Add => x.wrapping_add(y),
        Sub => x.wrapping_sub(y),
        Mul => x.wrapping_mul(y),
        Div => {
            if y == 0 {
                return None;
            }
            x.wrapping_div(y)
        }
        FloorDiv => {
            if y == 0 {
                return None;
            }
            let (q, r) = (x.wrapping_div(y), x.wrapping_rem(y));
            if r != 0 && (r < 0) != (y < 0) {
                q - 1
            } else {
                q
            }
        }
        Mod => {
            if y == 0 {
                return None;
            }
            let r = x.wrapping_rem(y);
            if r != 0 && (r < 0) != (y < 0) {
                r + y
            } else {
                r
            }
        }
        Shl => x.wrapping_shl(y as u32),
        Shr => x.wrapping_shr(y as u32),
        BitAnd => x & y,
        BitOr => x | y,
        BitXor => x ^ y,
    }))
}

fn fold_cmp(op: &mithril_front::ast::CmpOp, x: i64, y: i64) -> i64 {
    use mithril_front::ast::CmpOp::*;
    (match op {
        Lt => x < y,
        Le => x <= y,
        Gt => x > y,
        Ge => x >= y,
        Eq => x == y,
        Ne => x != y,
    }) as i64
}

struct Pe<'m> {
    m: &'m CoreModule,
    next: u32,
    lets: Vec<(u32, Core)>,
    iters: usize,
    size: usize,
    work: usize,
}

impl Pe<'_> {
    /// Residual value of `e` under `env` (callee var -> value); None aborts.
    fn val(&mut self, e: &Core, env: &mut HashMap<u32, Pv>) -> Option<Pv> {
        self.work += 1;
        if self.work > UNFOLD_WORK {
            return None;
        }
        Some(match e {
            Core::Num(n) => Pv::K(*n),
            Core::Var(i) => env.get(i)?.clone(),
            Core::Op2(op, a, b) => {
                let (x, y) = (self.val(a, env)?, self.val(b, env)?);
                match (&x, &y) {
                    (Pv::K(p), Pv::K(q)) => Pv::K(fold_op2(op, *p, *q)?),
                    _ => self.bind(Core::Op2(*op, Box::new(x.core()), Box::new(y.core()))),
                }
            }
            Core::Cmp(op, a, b) => {
                let (x, y) = (self.val(a, env)?, self.val(b, env)?);
                match (&x, &y) {
                    (Pv::K(p), Pv::K(q)) => Pv::K(fold_cmp(op, *p, *q)),
                    _ => self.bind(Core::Cmp(*op, Box::new(x.core()), Box::new(y.core()))),
                }
            }
            Core::If(c, t, f) => match self.val(c, env)? {
                Pv::K(k) => self.val(if k != 0 { t } else { f }, env)?,
                Pv::D(_) => return None, // dynamic control
            },
            Core::Let(x, r, b) => {
                let v = self.val(r, env)?;
                env.insert(*x, v);
                self.val(b, env)?
            }
            Core::Call(g, args) => {
                let vs: Option<Vec<Pv>> = args.iter().map(|a| self.val(a, env)).collect();
                self.call(*g, vs?)?
            }
            _ => return None, // data: not unfolded
        })
    }

    fn bind(&mut self, rhs: Core) -> Pv {
        self.size += 1;
        let v = self.next;
        self.next += 1;
        self.lets.push((v, rhs));
        Pv::D(Core::Var(v))
    }

    /// Unfold `g(args)`: its body under a fresh env; a self tail call is the
    /// next iteration of the same loop.
    fn call(&mut self, g: u32, args: Vec<Pv>) -> Option<Pv> {
        self.iters += 1;
        if self.iters > UNFOLD_CALLS || self.size > UNFOLD_SIZE {
            return None;
        }
        let f = &self.m.fns[g as usize];
        let mut env: HashMap<u32, Pv> = args.into_iter().enumerate().map(|(i, v)| (i as u32, v)).collect();
        self.val(&f.body, &mut env)
    }
}

/// `Some(residual)` when `Call(g, args)` (args evaluated in the caller)
/// unfolds; the residual's fresh binders start at `*next`.
fn unfold_call(m: &CoreModule, g: u32, args: &[Core], next: &mut u32) -> Option<Core> {
    if !args.iter().any(|a| matches!(a, Core::Num(_))) {
        return None;
    }
    let mut pe = Pe { m, next: *next, lets: Vec::new(), iters: 0, size: 0, work: 0 };
    // dynamic arguments are evaluated once, in the caller, as before
    let mut pre: Vec<(u32, Core)> = Vec::new();
    let mut vals = Vec::new();
    for a in args {
        match a {
            Core::Num(n) => vals.push(Pv::K(*n)),
            Core::Var(_) => vals.push(Pv::D(a.clone())),
            _ => {
                let v = pe.next;
                pe.next += 1;
                pre.push((v, a.clone()));
                vals.push(Pv::D(Core::Var(v)));
            }
        }
    }
    let res = pe.call(g, vals)?;
    if pe.size > UNFOLD_SIZE {
        return None;
    }
    *next = pe.next;
    let mut out = res.core();
    for (v, r) in pe.lets.into_iter().rev() {
        out = Core::Let(v, Box::new(r), Box::new(out));
    }
    for (v, r) in pre.into_iter().rev() {
        out = Core::Let(v, Box::new(r), Box::new(out));
    }
    Some(out)
}

/// Unfold every call with static control in every function (see above).
pub(crate) fn unfold_static(m: &CoreModule) -> CoreModule {
    fn go(e: &Core, m: &CoreModule, next: &mut u32) -> Core {
        let rec = |x: &Core, next: &mut u32| go(x, m, next);
        match e {
            Core::Call(g, args) => {
                let args: Vec<Core> = args.iter().map(|a| rec(a, next)).collect();
                unfold_call(m, *g, &args, next).unwrap_or(Core::Call(*g, args))
            }
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => e.clone(),
            Core::Op2(o, a, b) => Core::Op2(*o, Box::new(rec(a, next)), Box::new(rec(b, next))),
            Core::Cmp(o, a, b) => Core::Cmp(*o, Box::new(rec(a, next)), Box::new(rec(b, next))),
            Core::If(c, t, f) => Core::If(Box::new(rec(c, next)), Box::new(rec(t, next)), Box::new(rec(f, next))),
            Core::Let(x, r, b) => Core::Let(*x, Box::new(rec(r, next)), Box::new(rec(b, next))),
            Core::Ctor(c, xs) => Core::Ctor(*c, xs.iter().map(|x| rec(x, next)).collect()),
            Core::Reuse(v, c, xs) => Core::Reuse(*v, *c, xs.iter().map(|x| rec(x, next)).collect()),
            Core::Tuple(xs) => Core::Tuple(xs.iter().map(|x| rec(x, next)).collect()),
            Core::Proj(a, i) => Core::Proj(Box::new(rec(a, next)), *i),
            Core::Match(s, arms) => Core::Match(
                Box::new(rec(s, next)),
                arms.iter().map(|(c, bs, b)| (*c, bs.clone(), rec(b, next))).collect(),
            ),
        }
    }
    let mut out = m.clone();
    for f in out.fns.iter_mut() {
        let mut next = max_var(&f.body).max(f.arity as u32) + 1;
        f.body = go(&f.body, m, &mut next);
    }
    out
}
