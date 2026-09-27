//! Native scalar lowering.
//!
//! A function is *scalar* when every value it computes is an i56 integer:
//! `Num / Var / Op2 / Cmp / If / Let`, calls to other scalar functions, and —
//! because `while`/`for` desugar into helpers that tail-return a `Tuple` of
//! the live loop variables — an all-integer `Tuple` in tail position
//! (kind `SK(k)`), consumed at call sites exclusively through the
//! `Let(t, Call(g), .. Proj(Var t, i) ..)` idiom the desugarer emits.
//! No floats, constructors, matches, or escaping tuples.
//!
//! Scalar functions are emitted as plain `fn s_<fid>(v0: i64, ..) -> i64`
//! (or `-> (i64, .., i64)` for `SK(k)`) with native wrapping arithmetic (one
//! `wrap56` sign-fix per op, exactly matching `bin`'s int path),
//! self-tail-recursion as a loop, and no ctx/fuel/ownership plumbing. Their
//! `d_<fid>` dive form becomes a thin bridge (unpack ports -> call `s_` ->
//! repack; `SK` results build the same 0xFFF tuple cell the dive form
//! produced), so every existing call site — dive calls, CALL rules, fold
//! leaves — takes the fast path unchanged. Scalar calls consume no fuel:
//! they terminate by their own data-driven control flow, so a dive cannot
//! suspend inside one (suspension granularity coarsens by at most one scalar
//! call's work).
//!
//! Type-consistency note: the classifier proves int-ness from the callee's
//! side; a source program passing a non-NUM port into a scalar function gets
//! garbage arithmetic rather than a type error — identical to the
//! pre-lowering behavior of `bin`'s unchecked int path, and pinned by the
//! checksum oracles in every test lane.

use crate::seq::{bin_code, cmp_code};
use mithril_front::core::{Core, CoreModule};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Kind {
    No,
    S1,
    SK(usize),
}

/// Scalar parameter type: a bare i64, or a k-tuple of i64s passed as k
/// native components (read only through constant `Proj`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum PTy {
    I,
    T(usize),
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Sig {
    pub params: Vec<PTy>,
    pub ret: Kind, // S1 or SK(k); No never appears inside a Some(Sig)
}

/// Occurrences of `Var(t)` in `e` that are NOT directly under a `Proj`.
fn bare_uses(e: &Core, t: u32) -> usize {
    match e {
        Core::Var(i) => (*i == t) as usize,
        Core::Num(_) | Core::Flo(_) => 0,
        Core::Proj(b, _) => match &**b {
            Core::Var(i) if *i == t => 0,
            other => bare_uses(other, t),
        },
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => bare_uses(a, t) + bare_uses(b, t),
        Core::If(c, x, y) => bare_uses(c, t) + bare_uses(x, t) + bare_uses(y, t),
        Core::Let(_, r, b) => bare_uses(r, t) + bare_uses(b, t),
        Core::Call(_, args) | Core::Ctor(_, args) | Core::Tuple(args) | Core::Reuse(_, _, args) => {
            args.iter().map(|a| bare_uses(a, t)).sum()
        }
        Core::Match(s, arms) => {
            bare_uses(s, t) + arms.iter().map(|(_, _, b)| bare_uses(b, t)).sum::<usize>()
        }
    }
}

struct Chk<'m> {
    sigs: &'m [Option<Sig>],
    /// component vars: SK-destructured lets AND T(k) params -> arity
    tvars: HashMap<u32, usize>,
    ok: bool,
}

impl<'m> Chk<'m> {
    /// `e` is a plain i64-valued expression.
    fn expr(&mut self, e: &Core) {
        if !self.ok {
            return;
        }
        match e {
            Core::Num(_) => {}
            Core::Var(i) => {
                if self.tvars.contains_key(i) {
                    self.ok = false; // tuple var escaping without Proj
                }
            }
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
                self.expr(a);
                self.expr(b);
            }
            Core::If(c, x, y) => {
                self.expr(c);
                self.expr(x);
                self.expr(y);
            }
            Core::Let(x, r, b) => {
                self.bind(*x, r);
                self.expr(b);
            }
            Core::Call(g, args) => {
                match &self.sigs[*g as usize] {
                    Some(sig) if sig.ret == Kind::S1 => self.args(sig.params.clone(), args),
                    _ => self.ok = false,
                }
            }
            Core::Proj(b, i) => match &**b {
                Core::Var(t) if self.tvars.get(t).is_some_and(|k| i < k) => {}
                _ => self.ok = false,
            },
            _ => self.ok = false,
        }
    }

    /// Check call arguments against the callee's parameter types.
    fn args(&mut self, ptys: Vec<PTy>, args: &[Core]) {
        for (pt, a) in ptys.iter().zip(args) {
            match pt {
                PTy::I => self.expr(a),
                PTy::T(k) => match a {
                    Core::Var(t) if self.tvars.get(t) == Some(k) => {}
                    Core::Tuple(items) if items.len() == *k => {
                        for it in items {
                            self.expr(it);
                        }
                    }
                    _ => self.ok = false,
                },
            }
        }
    }

    /// A let binding: a plain scalar RHS, the SK-destructure idiom, or a
    /// tuple-valued expression (a join point: `if`/`match` arms that each
    /// yield a tuple of the variables they assign) bound as components.
    fn bind(&mut self, x: u32, r: &Core) {
        if let Some(k) = tuple_kind(self.sigs, r) {
            self.tuple_expr(r, k);
            self.tvars.insert(x, k);
            return;
        }
        self.expr(r);
    }

    /// A tuple-valued expression of `k` components.
    fn tuple_expr(&mut self, e: &Core, k: usize) {
        if !self.ok {
            return;
        }
        match e {
            Core::Tuple(items) if items.len() == k => items.iter().for_each(|it| self.expr(it)),
            Core::If(c, x, y) => {
                self.expr(c);
                self.tuple_expr(x, k);
                self.tuple_expr(y, k);
            }
            Core::Let(x, r, b) => {
                self.bind(*x, r);
                self.tuple_expr(b, k);
            }
            Core::Call(g, args) => match &self.sigs[*g as usize] {
                Some(sig) if sig.ret == Kind::SK(k) => self.args(sig.params.clone(), args),
                _ => self.ok = false,
            },
            _ => self.ok = false,
        }
    }

    /// Tail position; returns the tail kind (S1 / SK(k)) or sets !ok.
    fn tail(&mut self, e: &Core, fid: u32) -> Kind {
        if !self.ok {
            return Kind::No;
        }
        match e {
            Core::Let(x, r, b) => {
                self.bind(*x, r);
                self.tail(b, fid)
            }
            Core::If(c, x, y) => {
                self.expr(c);
                let a = self.tail(x, fid);
                let b = self.tail(y, fid);
                if a == b || b == Kind::No {
                    a
                } else if a == Kind::No {
                    b
                } else {
                    self.ok = false;
                    Kind::No
                }
            }
            Core::Tuple(items) => {
                for it in items {
                    self.expr(it);
                }
                Kind::SK(items.len())
            }
            Core::Call(g, args) => {
                match &self.sigs[*g as usize] {
                    Some(sig) => {
                        self.args(sig.params.clone(), args);
                        if *g == fid {
                            Kind::No // own kind, resolved by the caller of tail()
                        } else {
                            sig.ret
                        }
                    }
                    None => {
                        self.ok = false;
                        Kind::No
                    }
                }
            }
            other => {
                self.expr(other);
                Kind::S1
            }
        }
    }
}

/// Component count of a tuple-valued expression (`None` for scalars or
/// mixed shapes): a tuple literal, an `if` whose arms agree, a let chain
/// ending in one, or a call returning SK(k).
fn tuple_kind(sigs: &[Option<Sig>], e: &Core) -> Option<usize> {
    match e {
        Core::Tuple(items) => Some(items.len()),
        Core::If(_, x, y) => {
            let (a, b) = (tuple_kind(sigs, x), tuple_kind(sigs, y));
            if a == b {
                a
            } else {
                None
            }
        }
        Core::Let(_, _, b) => tuple_kind(sigs, b),
        Core::Call(g, _) => match &sigs[*g as usize] {
            Some(Sig { ret: Kind::SK(k), .. }) => Some(*k),
            _ => None,
        },
        _ => None,
    }
}

/// Max constant-Proj index observed on `Var(p)` in `e`, or None if `p` is
/// ever used bare (=> must be a plain int).
fn proj_shape(e: &Core, p: u32, bare: &mut bool, max: &mut i64) {
    match e {
        Core::Var(i) if *i == p => *bare = true,
        Core::Proj(b, i) => match &**b {
            Core::Var(t) if *t == p => *max = (*max).max(*i as i64),
            other => proj_shape(other, p, bare, max),
        },
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
            proj_shape(a, p, bare, max);
            proj_shape(b, p, bare, max);
        }
        Core::If(c, x, y) => {
            proj_shape(c, p, bare, max);
            proj_shape(x, p, bare, max);
            proj_shape(y, p, bare, max);
        }
        Core::Let(_, r, b) => {
            proj_shape(r, p, bare, max);
            proj_shape(b, p, bare, max);
        }
        Core::Call(_, a) | Core::Ctor(_, a) | Core::Tuple(a) | Core::Reuse(_, _, a) => {
            for x in a {
                proj_shape(x, p, bare, max);
            }
        }
        Core::Match(s, arms) => {
            proj_shape(s, p, bare, max);
            for (_, _, b) in arms {
                proj_shape(b, p, bare, max);
            }
        }
        _ => {}
    }
}

/// A tuple param's component count cannot be read off the body alone (a
/// caller may pass a wider tuple); seed with maxproj+1 and demote on caller
/// mismatch. Bare use of a param whose callers pass tuples also demotes.
pub(crate) fn classify(m: &CoreModule) -> Vec<Option<Sig>> {
    let n = m.fns.len();
    let mut sigs: Vec<Option<Sig>> = m
        .fns
        .iter()
        .map(|f| {
            let params = (0..f.arity as u32)
                .map(|p| {
                    let (mut bare, mut mx) = (false, -1i64);
                    proj_shape(&f.body, p, &mut bare, &mut mx);
                    if !bare && mx >= 0 {
                        PTy::T((mx + 1) as usize)
                    } else {
                        PTy::I
                    }
                })
                .collect();
            Some(Sig { params, ret: Kind::S1 })
        })
        .collect();
    // Param types are body-derived and fixed; rets/eligibility are
    // recomputed for EVERY fn each round (a demotion is not sticky: a fn
    // rejected while its callee's return kind was still a guess re-qualifies
    // once the callee settles). Bounded rounds; convergence break.
    let param_seed: Vec<Vec<PTy>> =
        sigs.iter().map(|s| s.as_ref().unwrap().params.clone()).collect();
    for _ in 0..2 * n + 4 {
        let mut next = sigs.clone();
        for (fid, f) in m.fns.iter().enumerate() {
            let params = param_seed[fid].clone();
            let ret_guess = sigs[fid].as_ref().map(|s| s.ret).unwrap_or(Kind::S1);
            let mut tv = HashMap::new();
            for (p, pt) in params.iter().enumerate() {
                if let PTy::T(k) = pt {
                    tv.insert(p as u32, *k);
                }
            }
            let mut c = Chk { sigs: &sigs, tvars: tv, ok: true };
            let tk = c.tail(&f.body, fid as u32);
            next[fid] = if !c.ok {
                None
            } else {
                let ret = match tk {
                    Kind::No => ret_guess, // only self tail calls: keep guess
                    k => k,
                };
                Some(Sig { params, ret })
            };
        }
        if next == sigs {
            break;
        }
        sigs = next;
    }
    // final verification (stale optimistic ret guesses)
    let snapshot = sigs.clone();
    for (fid, f) in m.fns.iter().enumerate() {
        let Some(sig) = &snapshot[fid] else { continue };
        let mut tv = HashMap::new();
        for (p, pt) in sig.params.iter().enumerate() {
            if let PTy::T(k) = pt {
                tv.insert(p as u32, *k);
            }
        }
        let mut c = Chk { sigs: &snapshot, tvars: tv, ok: true };
        let tk = c.tail(&f.body, fid as u32);
        let consistent = c.ok && matches!(tk, Kind::No) || (c.ok && tk == sig.ret);
        if !consistent {
            sigs[fid] = None;
        }
    }
    sigs
}

// ---------------------------------------------------------------- emission

struct Sem<'m> {
    sigs: &'m [Option<Sig>],
    tmp: u32,
    /// component vars (SK-destructured lets and T(k) params): var -> arity;
    /// components live as q<var>_<i>
    tvars: HashMap<u32, usize>,
}

impl<'m> Sem<'m> {
    fn fresh(&mut self) -> String {
        self.tmp += 1;
        format!("s{}", self.tmp)
    }

    /// The mutable local names of `fid`'s flattened parameter list, in
    /// flattened order (v<p> for ints, q<p>_<i> for tuple components).
    fn slot_names(&self, fid: u32) -> Vec<String> {
        let sig = self.sigs[fid as usize].as_ref().unwrap();
        let mut out = Vec::new();
        for (p, pt) in sig.params.iter().enumerate() {
            match pt {
                PTy::I => out.push(format!("v{p}")),
                PTy::T(k) => {
                    for i in 0..*k {
                        out.push(format!("q{p}_{i}"));
                    }
                }
            }
        }
        out
    }

    /// Flatten call args per the callee's parameter types (tuple params
    /// expand to k component expressions).
    fn call_args(&mut self, g: u32, args: &[Core], b: &mut String) -> Vec<String> {
        let sig = self.sigs[g as usize].clone().expect("call to non-scalar in scalar emission");
        let mut es = Vec::new();
        for (pt, a) in sig.params.iter().zip(args) {
            match pt {
                PTy::I => es.push(self.val(a, b)),
                PTy::T(k) => match a {
                    Core::Var(t) if self.tvars.contains_key(t) => {
                        for i in 0..*k {
                            es.push(format!("q{t}_{i}"));
                        }
                    }
                    Core::Tuple(items) => {
                        for it in items {
                            es.push(self.val(it, b));
                        }
                    }
                    _ => unreachable!("non-idiom tuple arg in scalar emission"),
                },
            }
        }
        es
    }

    fn bind(&mut self, x: u32, r: &Core, b: &mut String) {
        if let Some(k) = tuple_kind(self.sigs, r) {
            let et = self.tval(r, b);
            let comps: Vec<String> = (0..k).map(|i| format!("q{x}_{i}")).collect();
            b.push_str(&format!("let ({}) = {et};\n", comps.join(", ")));
            self.tvars.insert(x, k);
            return;
        }
        let er = self.val(r, b);
        b.push_str(&format!("let v{x} = {er};\n"));
    }

    /// A tuple-valued expression as a native Rust tuple expression.
    fn tval(&mut self, e: &Core, b: &mut String) -> String {
        match e {
            Core::Tuple(items) => {
                let es: Vec<String> = items.iter().map(|it| self.val(it, b)).collect();
                format!("({})", es.join(", "))
            }
            Core::If(c, x, y) => {
                let ec = self.val(c, b);
                let mut bx = String::new();
                let vx = self.tval(x, &mut bx);
                let mut by = String::new();
                let vy = self.tval(y, &mut by);
                format!("if {ec} != 0 {{\n{bx}{vx}\n}} else {{\n{by}{vy}\n}}")
            }
            Core::Let(x, r, bo) => {
                self.bind(*x, r, b);
                self.tval(bo, b)
            }
            Core::Call(g, args) => {
                let es = self.call_args(*g, args, b);
                format!("s_{g}({})", es.join(", "))
            }
            _ => unreachable!("non-tuple Core in scalar tuple emission"),
        }
    }

    fn val(&mut self, e: &Core, b: &mut String) -> String {
        match e {
            Core::Num(n) => format!("{n}i64"),
            Core::Var(i) => format!("v{i}"),
            Core::Proj(base, i) => match &**base {
                Core::Var(t) if self.tvars.contains_key(t) => format!("q{t}_{i}"),
                _ => unreachable!("non-idiom Proj in scalar emission"),
            },
            Core::Op2(op, x, y) => {
                let (ex, ey) = (self.val(x, b), self.val(y, b));
                let t = self.fresh();
                let body = match bin_code(op) {
                    0 => format!("{ex}.wrapping_add({ey})"),
                    1 => format!("{ex}.wrapping_sub({ey})"),
                    2 => format!("{ex}.wrapping_mul({ey})"),
                    3 => format!("{ex}.wrapping_div({ey})"),
                    4 => format!("floor_div({ex}, {ey})"),
                    5 => format!("py_mod({ex}, {ey})"),
                    6 => format!("{ex}.wrapping_shl({ey} as u32)"),
                    7 => format!("{ex}.wrapping_shr({ey} as u32)"),
                    8 => format!("{ex} & {ey}"),
                    9 => format!("{ex} | {ey}"),
                    _ => format!("{ex} ^ {ey}"),
                };
                b.push_str(&format!("let {t} = wrap56({body});\n"));
                t
            }
            Core::Cmp(op, x, y) => {
                let (ex, ey) = (self.val(x, b), self.val(y, b));
                let o = match cmp_code(op) {
                    0 => "<",
                    1 => "<=",
                    2 => ">",
                    3 => ">=",
                    4 => "==",
                    _ => "!=",
                };
                format!("(({ex} {o} {ey}) as i64)")
            }
            Core::If(c, x, y) => {
                let ec = self.val(c, b);
                let mut bx = String::new();
                let vx = self.val(x, &mut bx);
                let mut by = String::new();
                let vy = self.val(y, &mut by);
                let t = self.fresh();
                b.push_str(&format!(
                    "let {t} = if {ec} != 0 {{\n{bx}{vx}\n}} else {{\n{by}{vy}\n}};\n"
                ));
                t
            }
            Core::Let(x, r, bo) => {
                self.bind(*x, r, b);
                self.val(bo, b)
            }
            Core::Call(g, args) => {
                let es = self.call_args(*g, args, b);
                format!("s_{g}({})", es.join(", "))
            }
            _ => unreachable!("non-scalar Core in scalar emission"),
        }
    }

    /// Tail position: self tail calls become loop iterations; everything
    /// else returns (a bare i64 for S1, a native tuple for SK).
    fn tail(&mut self, e: &Core, fid: u32, lp: bool, b: &mut String) {
        match e {
            Core::Let(x, r, bo) => {
                self.bind(*x, r, b);
                self.tail(bo, fid, lp, b);
            }
            Core::If(c, x, y) => {
                let ec = self.val(c, b);
                b.push_str(&format!("if {ec} != 0 {{\n"));
                self.tail(x, fid, lp, b);
                b.push_str("} else {\n");
                self.tail(y, fid, lp, b);
                b.push_str("}\n");
            }
            Core::Call(g, args) if *g == fid && lp => {
                let es = self.call_args(*g, args, b);
                for (i, ea) in es.iter().enumerate() {
                    b.push_str(&format!("let n{i} = {ea};\n"));
                }
                for (i, slot) in self.slot_names(fid).into_iter().enumerate() {
                    b.push_str(&format!("{slot} = n{i};\n"));
                }
                b.push_str("continue 'l;\n");
            }
            Core::Call(g, args) => {
                let es = self.call_args(*g, args, b);
                b.push_str(&format!("return s_{g}({});\n", es.join(", ")));
            }
            Core::Tuple(items) => {
                let es: Vec<String> = items.iter().map(|a| self.val(a, b)).collect();
                b.push_str(&format!("return ({});\n", es.join(", ")));
            }
            other => {
                let v = self.val(other, b);
                b.push_str(&format!("return {v};\n"));
            }
        }
    }
}

/// The native form `s_<fid>` plus the bridging dive form `d_<fid>`.
/// `bor[fid][p]` = param p is borrowed (bridge must not free a tuple arg's
/// spine; owned tuple args are freed after unpacking — components are NUMs,
/// so only the spine cells matter).
pub(crate) fn scalar_fn(m: &CoreModule, fid: u32, sigs: &[Option<Sig>], bor: &[Vec<bool>]) -> String {
    let f = &m.fns[fid as usize];
    let ar = f.arity;
    let lp = f.self_tail_rec;
    let sig = sigs[fid as usize].as_ref().unwrap().clone();
    let mut params: Vec<String> = Vec::new();
    let mw = if lp { "mut " } else { "" };
    for (p, pt) in sig.params.iter().enumerate() {
        match pt {
            PTy::I => params.push(format!("{mw}v{p}: i64")),
            PTy::T(k) => {
                for i in 0..*k {
                    params.push(format!("{mw}q{p}_{i}: i64"));
                }
            }
        }
    }
    let ret = match sig.ret {
        Kind::S1 => "i64".to_string(),
        Kind::SK(k) => format!("({})", vec!["i64"; k].join(", ")),
        Kind::No => unreachable!(),
    };
    let mut tv = HashMap::new();
    for (p, pt) in sig.params.iter().enumerate() {
        if let PTy::T(k) = pt {
            tv.insert(p as u32, *k);
        }
    }
    let mut sem = Sem { sigs, tmp: 0, tvars: tv };
    let mut bb = String::new();
    sem.tail(&f.body, fid, lp, &mut bb);
    let body = if lp { format!("'l: loop {{\n{bb}}}\n") } else { bb };

    // bridge: unpack ports per param type, call, repack per return kind
    let mut unpack = String::new();
    let mut bargs: Vec<String> = Vec::new();
    let mut needs_cell_read = false;
    for (p, pt) in sig.params.iter().enumerate() {
        match pt {
            PTy::I => bargs.push(format!("as_i(v{p})")),
            PTy::T(k) => {
                needs_cell_read = true;
                for i in 0..*k {
                    unpack.push_str(&format!("let a{p}_{i} = as_i(field(ctx, v{p}, {i}));\n"));
                    bargs.push(format!("a{p}_{i}"));
                }
                if !bor[fid as usize][p] {
                    unpack.push_str(&format!("free_val(ctx, v{p});\n"));
                }
            }
        }
    }
    let owns_tuple = sig.params.iter().enumerate().any(|(p, pt)| matches!(pt, PTy::T(_)) && !bor[fid as usize][p]);
    let bridge_body = match sig.ret {
        Kind::S1 => format!("{unpack}Ok(num(s_{fid}({})))", bargs.join(", ")),
        Kind::SK(k) => {
            let comps: Vec<String> = (0..k).map(|i| format!("r{i}")).collect();
            let packs: Vec<String> = (0..k).map(|i| format!("num(r{i})")).collect();
            format!(
                "{unpack}let ({}) = s_{fid}({});\nOk(mk_con(ctx, 0xFFFu16, &[{}]))",
                comps.join(", "),
                bargs.join(", "),
                packs.join(", ")
            )
        }
        Kind::No => unreachable!(),
    };
    let cx = if needs_cell_read || owns_tuple || matches!(sig.ret, Kind::SK(_)) { "ctx" } else { "_ctx" };
    format!(
        "#[allow(unused_mut, unused_variables, clippy::let_and_return, clippy::too_many_arguments)]\n\
         fn s_{fid}({}) -> {ret} {{\n{body}}}\n\n\
         #[allow(unused_variables, clippy::too_many_arguments)]\n\
         fn d_{fid}({cx}: &mut Wctx, _fuel: &mut i64{}) -> R {{\n\
         {bridge_body}\n}}\n\n",
        params.join(", "),
        (0..ar).map(|i| format!(", v{i}: u64")).collect::<String>(),
    )
}
