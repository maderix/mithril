//! Dive-form emission (Core -> sequential Rust) and the expression emitter
//! shared with the rule form. Runtime values are Port raws (`u64`): NUM i56
//! immediates, CON cells (chained past arity 2), boxed floats. Ctor payloads
//! are read via `ctx.cell` (through the generated `field` helper), so no
//! readback is ever needed mid-run.
//!
//! # Linear cell discipline
//!
//! Values are consumed linearly: each function owns its (non-borrowed)
//! arguments, a `match`/`Proj` on the *last use* of a variable frees the
//! constructor spine (tree_drop-on-match), non-last uses of owned variables
//! deep-copy (`dup_val`), and read-only ("borrowed") parameters — inferred in
//! `lib.rs` — are passed raw, never freed by the callee, and reclaimed by the
//! caller (dive) or the CALL rule (fire) after completion. All frees inside a
//! dive are *deferred* into `fr` and applied only when the dive completes or
//! commits, so a fuel-out unwind never invalidates the original arguments.

use crate::rules::{emit_rec, emit_spawn, fork_parts, SegQ};
use crate::{cnt_dive, free_vars, has_call, Cnt};
use mithril_front::ast::{BinOp, CmpOp};
use mithril_front::core::{Core, CoreModule, UNREACHABLE_CTOR};
use std::collections::HashSet;

pub(crate) fn bin_code(op: &BinOp) -> u8 {
    match op {
        BinOp::Add => 0,
        BinOp::Sub => 1,
        BinOp::Mul => 2,
        BinOp::Div => 3,
        BinOp::FloorDiv => 4,
        BinOp::Mod => 5,
        BinOp::Shl => 6,
        BinOp::Shr => 7,
        BinOp::BitAnd => 8,
        BinOp::BitOr => 9,
        BinOp::BitXor => 10,
    }
}

pub(crate) fn cmp_code(op: &CmpOp) -> u8 {
    match op {
        CmpOp::Lt => 0,
        CmpOp::Le => 1,
        CmpOp::Gt => 2,
        CmpOp::Ge => 3,
        CmpOp::Eq => 4,
        CmpOp::Ne => 5,
    }
}

/// How a match/proj scrutinee is held.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Hold {
    /// Owned last use: extract fields, free the spine.
    Consume,
    /// Shared owned value: dup extracted fields, leave the spine.
    BorrowDup,
    /// Borrow-derived value: raw field reads, owner is upstream.
    BorrowRaw,
}

/// Statement-oriented expression emitter. `dive` mode compiles calls as
/// nested `d_*` dives; rule mode must never see a call in expression
/// position (the body is ANF-normalized first).
pub(crate) struct Ex<'m> {
    pub tmp: u32,
    pub dive: bool,
    pub self_fid: u32,
    pub loop_form: bool,
    /// Remaining-use counts of owned variables (branch-aware max).
    pub rem: Cnt,
    /// >0 inside value-position If/Match branches: reads dup, never move.
    pub pinned: u32,
    /// Borrow-derived variables (dive mode only; empty in rule mode).
    pub bset: HashSet<u32>,
    /// Per-function parameter borrow modes.
    pub bor: &'m [Vec<bool>],
    /// Segment registry (dive mode only): fuel-out captures continuations
    /// as segments exactly like the rule form.
    pub sq: Option<&'m mut SegQ>,
    /// Enclosing value-position let continuations (x, body) whose RHS is
    /// mid-evaluation; a suspension inside chains records through them.
    pub kframes: Vec<(u32, Core)>,
    /// The forwarding segment id (delivers its single slot to its parent).
    pub fwd: u16,
    /// Variables proven to hold i56 immediates (see `numeric_vars`): their
    /// dup/free are elided and arithmetic on them is emitted inline.
    pub ints: HashSet<u32>,
}

/// Variables that are *used* as arithmetic/comparison operands or bound to
/// arithmetic results. Sound as an i56 proof only when the module contains
/// no float literal (floats cannot arise otherwise), which the caller
/// checks; returns an empty set when the proof does not hold.
pub(crate) fn numeric_vars(body: &Core, float_free: bool) -> HashSet<u32> {
    fn is_numeric_expr(e: &Core) -> bool {
        matches!(e, Core::Num(_) | Core::Op2(..) | Core::Cmp(..))
    }
    fn walk(e: &Core, out: &mut HashSet<u32>) {
        match e {
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
                for x in [a, b] {
                    if let Core::Var(i) = &**x {
                        out.insert(*i);
                    }
                    walk(x, out);
                }
            }
            Core::If(c, t, f) => {
                if let Core::Var(i) = &**c {
                    out.insert(*i);
                }
                walk(c, out);
                walk(t, out);
                walk(f, out);
            }
            Core::Let(x, r, b) => {
                if is_numeric_expr(r) {
                    out.insert(*x);
                }
                walk(r, out);
                walk(b, out);
            }
            Core::Call(_, a) | Core::Ctor(_, a) | Core::Tuple(a) => {
                for x in a {
                    walk(x, out);
                }
            }
            Core::Match(s, arms) => {
                walk(s, out);
                for (_, _, b) in arms {
                    walk(b, out);
                }
            }
            Core::Proj(b, _) => walk(b, out),
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => {}
        }
    }
    let mut out = HashSet::new();
    if float_free {
        walk(body, &mut out);
    }
    out
}

impl<'m> Ex<'m> {
    pub fn new(
        dive: bool,
        self_fid: u32,
        loop_form: bool,
        rem: Cnt,
        bset: HashSet<u32>,
        bor: &'m [Vec<bool>],
        ints: HashSet<u32>,
        sq: Option<&'m mut SegQ>,
        fwd: u16,
    ) -> Ex<'m> {
        Ex { tmp: 0, dive, self_fid, loop_form, rem, pinned: 0, bset, bor, ints, sq, kframes: Vec::new(), fwd }
    }

    /// Emit the suspension path for a dive call whose result `rv` (a record
    /// awaiting its parent) came back as `Err`: build the continuation
    /// record chain — `own` (this let's continuation) then every enclosing
    /// value-position frame — patch parents inward, and return the
    /// outermost record for the caller to attach.
    pub(crate) fn emit_capture(&mut self, rv: &str, own: Option<(u32, &Core)>, b: &mut String) {
        let saved = self.rem.clone();
        let mut frames: Vec<(u32, Core)> = Vec::new();
        if let Some((x, bo)) = own {
            frames.push((x, bo.clone()));
        }
        for f in self.kframes.iter().rev() {
            frames.push(f.clone());
        }
        let mut child = rv.to_string();
        // Fork-shaped innermost continuation (`x = g(..); y = h(..)` with h
        // independent of x): the suspended g's sibling h has not started —
        // spawn it now behind a pend-2 record so the fork runs in parallel,
        // exactly like the rule form's pair fork.
        if let Some((x, bo)) = own {
            if let Some((y, h, hargs, bo2)) = fork_parts(&x, bo) {
                let mut env = free_vars(bo2);
                env.remove(&x);
                env.remove(y);
                let env: Vec<u32> = env.into_iter().collect();
                let sid = self.sq.as_mut().expect("dive capture without a segment registry").add(vec![x, *y], env.clone(), bo2.clone());
                let rn = emit_rec(self, &env, sid, 2, "NONE", b);
                b.push_str(&format!("ctx.set_parent({child}, ({rn} as u64) << 3);\n"));
                emit_spawn(self, *h, hargs, &format!("((({rn} as u64) << 3) | 1)"), b);
                child = rn;
                frames.remove(0);
            }
        }
        for (x, bo) in frames {
            let mut env = free_vars(&bo);
            env.remove(&x);
            let env: Vec<u32> = env.into_iter().collect();
            let sid = self.sq.as_mut().expect("dive capture without a segment registry").add(vec![x], env.clone(), bo);
            let rn = emit_rec(self, &env, sid, 1, "NONE", b);
            b.push_str(&format!("ctx.set_parent({child}, ({rn} as u64) << 3);\n"));
            child = rn;
        }
        b.push_str(&format!("return Err({child});\n"));
        self.rem = saved;
    }

    /// A let whose RHS is a call: run the dive, capturing on suspension.
    fn let_call(&mut self, x: u32, g: u32, args: &[Core], bo: &Core, b: &mut String) {
        let (call, post) = self.dive_call(g, args, b);
        let t = self.fresh();
        b.push_str(&format!("let {t} = match {call} {{\nOk(v) => v,\nErr(r) => {{\n"));
        self.emit_capture("r", Some((x, bo)), b);
        b.push_str("}\n};\n");
        for p in post {
            b.push_str(&format!("free_val(ctx, {p});\n"));
        }
        self.emit_bind(x, &t, b);
    }

    /// An expression whose runtime value is a proven i56 immediate.
    fn is_int(&self, e: &Core) -> bool {
        match e {
            Core::Num(_) | Core::Op2(..) | Core::Cmp(..) => true,
            Core::Var(i) => self.ints.contains(i),
            _ => false,
        }
    }

    pub fn fresh(&mut self) -> String {
        self.tmp += 1;
        format!("t{}", self.tmp)
    }

    fn is_braw(&self, e: &Core) -> bool {
        self.dive && matches!(e, Core::Var(i) if self.bset.contains(i))
    }

    /// Read variable `i`. `esc` = the value escapes into a structure /
    /// owned call / result, so a borrow-derived read must be deep-copied.
    pub fn use_var(&mut self, i: u32, esc: bool, b: &mut String) -> String {
        if self.ints.contains(&i) {
            // immediates: no ownership, no copy
            if let Some(r) = self.rem.get_mut(&i) {
                *r = (*r - 1).max(0);
            }
            return format!("v{i}");
        }
        if self.dive && self.bset.contains(&i) {
            if esc {
                let t = self.fresh();
                b.push_str(&format!("let {t} = dup_val(ctx, v{i});\n"));
                return t;
            }
            return format!("v{i}");
        }
        if self.pinned > 0 {
            let t = self.fresh();
            b.push_str(&format!("let {t} = dup_val(ctx, v{i});\n"));
            return t;
        }
        match self.rem.get_mut(&i) {
            Some(r) if *r >= 2 => {
                *r -= 1;
                let t = self.fresh();
                b.push_str(&format!("let {t} = dup_val(ctx, v{i});\n"));
                t
            }
            Some(r) if *r == 1 => {
                *r = 0;
                format!("v{i}")
            }
            _ => {
                let t = self.fresh();
                b.push_str(&format!("let {t} = dup_val(ctx, v{i});\n"));
                t
            }
        }
    }

    /// Bind `let v{x} = expr;` and free it right away if it is never used.
    fn emit_bind(&mut self, x: u32, expr: &str, b: &mut String) {
        if self.ints.contains(&x) {
            b.push_str(&format!("let v{x} = {expr};\n"));
            return;
        }
        if !self.bset.contains(&x) && self.rem.get(&x).copied().unwrap_or(0) == 0 {
            b.push_str(&format!("free_val(ctx, {expr});\n"));
        } else {
            b.push_str(&format!("let v{x} = {expr};\n"));
        }
    }

    /// Evaluate a match/proj scrutinee and decide its hold mode.
    pub(crate) fn scrutinee(&mut self, s: &Core, b: &mut String) -> (String, Hold) {
        if let Core::Var(i) = s {
            if self.dive && self.bset.contains(i) {
                return (format!("v{i}"), Hold::BorrowRaw);
            }
            if self.pinned > 0 {
                return (format!("v{i}"), Hold::BorrowDup);
            }
            return match self.rem.get_mut(i) {
                Some(r) if *r >= 2 => {
                    *r -= 1;
                    (format!("v{i}"), Hold::BorrowDup)
                }
                Some(r) if *r == 1 => {
                    *r = 0;
                    (format!("v{i}"), Hold::Consume)
                }
                _ => (format!("v{i}"), Hold::BorrowDup),
            };
        }
        let e = self.val(s, false, b);
        (e, Hold::Consume)
    }

    /// Bind a match arm's fields under the scrutinee's hold mode; Consume
    /// also frees the constructor spine.
    pub(crate) fn bind_fields(
        &mut self,
        sv: &str,
        hold: Hold,
        binders: &[u32],
        b: &mut String,
    ) {
        for (i, bv) in binders.iter().enumerate() {
            let cnt = self.rem.get(bv).copied().unwrap_or(0);
            match hold {
                Hold::BorrowRaw => {
                    b.push_str(&format!("let v{bv} = field(ctx, {sv}, {i});\n"));
                }
                Hold::Consume => {
                    if cnt == 0 {
                        b.push_str(&format!("free_val(ctx, field(ctx, {sv}, {i}));\n"));
                    } else {
                        b.push_str(&format!("let v{bv} = field(ctx, {sv}, {i});\n"));
                    }
                }
                Hold::BorrowDup => {
                    if cnt > 0 || self.pinned > 0 {
                        b.push_str(&format!(
                            "let v{bv} = dup_val(ctx, field(ctx, {sv}, {i}));\n"
                        ));
                    }
                }
            }
        }
        if hold == Hold::Consume {
            b.push_str(&format!("free_spine(ctx, {sv});\n"));
        }
    }

    /// A dive call `d_g(...)`; returns (call expression, frees to run after
    /// the call succeeds: owned values lent to borrowed parameters).
    fn dive_call(
        &mut self,
        g: u32,
        args: &[Core],
        b: &mut String,
    ) -> (String, Vec<String>) {
        assert!(self.dive, "codegen bug: call in a pure rule-form expression");
        let modes = &self.bor[g as usize];
        let mut post = Vec::new();
        let mut es = Vec::new();
        for (j, a) in args.iter().enumerate() {
            if modes[j] {
                // borrowed parameter: lend a read
                match a {
                    Core::Var(i) if self.bset.contains(i) => es.push(format!("v{i}")),
                    Core::Var(i) => {
                        if self.pinned > 0 {
                            es.push(format!("v{i}"));
                        } else {
                            match self.rem.get_mut(i) {
                                Some(r) if *r >= 2 => {
                                    *r -= 1;
                                    es.push(format!("v{i}"));
                                }
                                Some(r) if *r == 1 => {
                                    // last use: we still own it after the call
                                    *r = 0;
                                    es.push(format!("v{i}"));
                                    post.push(format!("v{i}"));
                                }
                                _ => es.push(format!("v{i}")),
                            }
                        }
                    }
                    _ => {
                        let t = self.val(a, true, b);
                        post.push(t.clone());
                        es.push(t);
                    }
                }
            } else {
                es.push(self.val(a, true, b));
            }
        }
        let argl: String = es.iter().map(|e| format!(", {e}")).collect();
        (format!("d_{g}(ctx, fuel{argl})"), post)
    }

    /// Emit statements computing `e` into `b`; returns a Rust expression
    /// (temp name, local, or literal) holding the value.
    pub fn val(&mut self, e: &Core, esc: bool, b: &mut String) -> String {
        match e {
            Core::Num(n) => format!("num({}i64)", n),
            Core::Flo(x) => {
                let t = self.fresh();
                b.push_str(&format!(
                    "let {t} = flo(ctx, f64::from_bits(0x{:016x}u64));\n",
                    x.to_bits()
                ));
                t
            }
            Core::Var(i) => self.use_var(*i, esc, b),
            Core::Op2(op, x, y) => {
                let ints = self.is_int(x) && self.is_int(y);
                let own = (!self.is_braw(x) as u8) | ((!self.is_braw(y) as u8) << 1);
                let ex = self.val(x, false, b);
                let ey = self.val(y, false, b);
                let t = self.fresh();
                if ints {
                    let (a, c) = (format!("as_i({ex})"), format!("as_i({ey})"));
                    let body = match bin_code(op) {
                        0 => format!("{a}.wrapping_add({c})"),
                        1 => format!("{a}.wrapping_sub({c})"),
                        2 => format!("{a}.wrapping_mul({c})"),
                        3 => format!("{a}.wrapping_div({c})"),
                        4 => format!("floor_div({a}, {c})"),
                        5 => format!("py_mod({a}, {c})"),
                        6 => format!("{a}.wrapping_shl({c} as u32)"),
                        7 => format!("{a}.wrapping_shr({c} as u32)"),
                        8 => format!("{a} & {c}"),
                        9 => format!("{a} | {c}"),
                        _ => format!("{a} ^ {c}"),
                    };
                    b.push_str(&format!("let {t} = num(wrap56({body}));\n"));
                } else {
                    b.push_str(&format!(
                        "let {t} = bin(ctx, {}u8, {ex}, {ey}, {own}u8);\n",
                        bin_code(op)
                    ));
                }
                t
            }
            Core::Cmp(op, x, y) => {
                let ints = self.is_int(x) && self.is_int(y);
                let own = (!self.is_braw(x) as u8) | ((!self.is_braw(y) as u8) << 1);
                let ex = self.val(x, false, b);
                let ey = self.val(y, false, b);
                let t = self.fresh();
                if ints {
                    let o = match cmp_code(op) {
                        0 => "<",
                        1 => "<=",
                        2 => ">",
                        3 => ">=",
                        4 => "==",
                        _ => "!=",
                    };
                    b.push_str(&format!("let {t} = num((as_i({ex}) {o} as_i({ey})) as i64);\n"));
                } else {
                    b.push_str(&format!(
                        "let {t} = cmp(ctx, {}u8, {ex}, {ey}, {own}u8);\n",
                        cmp_code(op)
                    ));
                }
                t
            }
            Core::If(c, th, el) => {
                let ec = self.val(c, false, b);
                self.pinned += 1;
                let bt = self.block_val(th, esc);
                let bf = self.block_val(el, esc);
                self.pinned -= 1;
                let t = self.fresh();
                b.push_str(&format!(
                    "let {t} = if as_i({ec}) != 0 {{\n{bt}}} else {{\n{bf}}};\n"
                ));
                t
            }
            Core::Let(x, r, bo) => {
                if self.dive {
                    if let Core::Call(g, args) = &**r {
                        self.let_call(*x, *g, args, bo, b);
                        return self.val(bo, esc, b);
                    }
                    if has_call(r) {
                        self.kframes.push((*x, (**bo).clone()));
                        let er = self.val(r, false, b);
                        self.kframes.pop();
                        self.emit_bind(*x, &er, b);
                        return self.val(bo, esc, b);
                    }
                }
                let er = self.val(r, false, b);
                self.emit_bind(*x, &er, b);
                self.val(bo, esc, b)
            }
            Core::Call(g, args) => {
                // Only reachable for a call in bare value position, which
                // ANF forbids; the tail/let paths own every real call site.
                let (call, post) = self.dive_call(*g, args, b);
                let t = self.fresh();
                b.push_str(&format!("let {t} = match {call} {{\nOk(v) => v,\nErr(r) => {{\n"));
                self.emit_capture("r", None, b);
                b.push_str("}\n};\n");
                for p in post {
                    b.push_str(&format!("free_val(ctx, {p});\n"));
                }
                t
            }
            Core::Ctor(cid, args) => {
                if *cid == UNREACHABLE_CTOR {
                    let t = self.fresh();
                    b.push_str(&format!("let {t} = mith_unreachable();\n"));
                    return t;
                }
                assert!(*cid < 0xFFE, "codegen: ctor id {} collides with reserved tags", cid);
                let es: Vec<String> = args.iter().map(|a| self.val(a, true, b)).collect();
                let t = self.fresh();
                b.push_str(&mk_con_call(&t, *cid as u32, &es));
                t
            }
            Core::Tuple(items) => {
                let es: Vec<String> = items.iter().map(|a| self.val(a, true, b)).collect();
                let t = self.fresh();
                b.push_str(&mk_con_call(&t, 0xFFF, &es));
                t
            }
            Core::Proj(x, i) => {
                let (sv, hold) = self.scrutinee(x, b);
                let t = self.fresh();
                if hold == Hold::Consume {
                    b.push_str(&format!("let {t} = take_field(ctx, {sv}, {i});\n"));
                } else {
                    b.push_str(&format!("let {t} = dup_val(ctx, field(ctx, {sv}, {i}));\n"));
                }
                t
            }
            Core::Match(s, arms) => {
                let (sv, hold) = self.scrutinee(s, b);
                let t = self.fresh();
                let mut code = format!("let {t} = match con_tag({sv}) {{\n");
                self.pinned += 1;
                for (cid, binders, body) in arms {
                    if *cid == UNREACHABLE_CTOR {
                        continue;
                    }
                    let mut ab = String::new();
                    self.bind_fields(&sv, hold, binders, &mut ab);
                    let bb = self.block_val(body, esc);
                    code.push_str(&format!("{cid} => {{\n{ab}{bb}}}\n"));
                }
                self.pinned -= 1;
                code.push_str("_ => mith_unreachable(),\n};\n");
                b.push_str(&code);
                t
            }
        }
    }

    /// `e` as a block body: statements plus a trailing value expression.
    pub fn block_val(&mut self, e: &Core, esc: bool) -> String {
        let mut s = String::new();
        let v = self.val(e, esc, &mut s);
        s.push_str(&v);
        s.push('\n');
        s
    }

    /// Emit `e` in dive tail position: ends every path with `return`, or
    /// `continue 'l` for self tail calls in loop form.
    pub fn dive_tail(&mut self, e: &Core, b: &mut String) {
        match e {
            Core::Let(x, r, bo) => {
                if let Core::Call(g, args) = &**r {
                    self.let_call(*x, *g, args, bo, b);
                    return self.dive_tail(bo, b);
                }
                if has_call(r) {
                    self.kframes.push((*x, (**bo).clone()));
                    let er = self.val(r, false, b);
                    self.kframes.pop();
                    self.emit_bind(*x, &er, b);
                    return self.dive_tail(bo, b);
                }
                let er = self.val(r, false, b);
                self.emit_bind(*x, &er, b);
                self.dive_tail(bo, b);
            }
            Core::If(c, th, el) => {
                let ec = self.val(c, false, b);
                let saved = self.rem.clone();
                b.push_str(&format!("if as_i({ec}) != 0 {{\n"));
                self.dive_tail(th, b);
                self.rem = saved.clone();
                b.push_str("} else {\n");
                self.dive_tail(el, b);
                self.rem = saved;
                b.push_str("}\n");
            }
            Core::Match(s, arms) => {
                let (sv, hold) = self.scrutinee(s, b);
                let saved = self.rem.clone();
                b.push_str(&format!("match con_tag({sv}) {{\n"));
                for (cid, binders, body) in arms {
                    if *cid == UNREACHABLE_CTOR {
                        continue;
                    }
                    self.rem = saved.clone();
                    b.push_str(&format!("{cid} => {{\n"));
                    self.bind_fields(&sv, hold, binders, b);
                    self.dive_tail(body, b);
                    b.push_str("}\n");
                }
                self.rem = saved;
                b.push_str("_ => { mith_unreachable(); }\n}\n");
            }
            Core::Call(g, args) if *g == self.self_fid && self.loop_form => {
                // Self tail call as a loop iteration: compute all next-state
                // values first (they read the current v*), then assign.
                // A borrow-derived value passed through a borrowed self
                // parameter stays raw (same frame, owner unchanged).
                let modes = self.bor[*g as usize].clone();
                let es: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(j, a)| {
                        if modes[j] && self.is_braw(a) {
                            if let Core::Var(i) = a {
                                format!("v{i}")
                            } else {
                                unreachable!()
                            }
                        } else {
                            self.val(a, true, b)
                        }
                    })
                    .collect();
                for (i, ea) in es.iter().enumerate() {
                    b.push_str(&format!("let n{i} = {ea};\n"));
                }
                for i in 0..es.len() {
                    b.push_str(&format!("v{i} = n{i};\n"));
                }
                b.push_str("continue 'l;\n");
            }
            Core::Call(g, args) => {
                // Tail call: pass our own destination through, so a downstream
                // suspension spawns its pending call against the right parent.
                let (call, post) = self.dive_call(*g, args, b);
                if post.is_empty() {
                    b.push_str(&format!("return {call};\n"));
                } else {
                    b.push_str(&format!("let tr = {call};\nif tr.is_ok() {{\n"));
                    for p in post {
                        b.push_str(&format!("free_val(ctx, {p});\n"));
                    }
                    b.push_str("}\nreturn tr;\n");
                }
            }
            other => {
                let v = self.val(other, true, b);
                b.push_str(&format!("return Ok({v});\n"));
            }
        }
    }
}

/// Constructor allocation call: slice-free fast paths for arity 1 and 2.
pub(crate) fn mk_con_call(t: &str, cid: u32, es: &[String]) -> String {
    match es.len() {
        1 => format!("let {t} = mk_con1(ctx, {cid}u16, {});\n", es[0]),
        2 => format!("let {t} = mk_con2(ctx, {cid}u16, {}, {});\n", es[0], es[1]),
        _ => format!("let {t} = mk_con(ctx, {cid}u16, &[{}]);\n", es.join(", ")),
    }
}

/// The dive form of function `fid`: fuel per call / loop iteration; on
/// exhaustion with a known destination the pending call is spawned as a
/// redex (suspension), otherwise the dive unwinds and the caller falls back
/// to the rule form.
pub(crate) fn dive_fn<'m>(
    m: &CoreModule,
    fid: u32,
    body: &Core,
    bor: &'m [Vec<bool>],
    bset: &HashSet<u32>,
    sq: &'m mut SegQ,
    fwd: u16,
) -> String {
    let f = &m.fns[fid as usize];
    let ar = f.arity;
    let lp = f.self_tail_rec;
    let params: String =
        (0..ar).map(|i| format!(", {}v{}: u64", if lp { "mut " } else { "" }, i)).collect();
    let argl = (0..ar).map(|i| format!("v{i}")).collect::<Vec<_>>().join(", ");
    // Fuel-out at entry / loop top: the pending call IS the continuation;
    // spawn it against a forwarding record whose parent the caller sets.
    let fuel_check = format!(
        "*fuel -= 1;\nif *fuel < 0 {{\nlet r = ctx.alloc_rec({fwd}u16, 1, 0, 0, NONE);\nspawn_call(ctx, {}u16, &[{argl}], (r as u64) << 3);\nreturn Err(r);\n}}\n",
        1 + fid
    );
    let mut rem = Cnt::new();
    cnt_dive(body, &mut rem);
    let ints = numeric_vars(body, crate::float_free(m));
    let mut ex = Ex::new(true, fid, lp, rem, bset.clone(), bor, ints, Some(sq), fwd);
    let mut bb = String::new();
    if !lp {
        // Owned parameters that the body never reads die immediately.
        for i in 0..ar as u32 {
            if !ex.bset.contains(&i) && ex.rem.get(&i).copied().unwrap_or(0) == 0 {
                bb.push_str(&format!("free_val(ctx, v{i});\n"));
            }
        }
    }
    ex.dive_tail(body, &mut bb);
    let mut s = format!(
        "#[allow(clippy::too_many_arguments)]\nfn d_{fid}(ctx: &mut Wctx, fuel: &mut i64{params}) -> R {{\n"
    );
    if lp {
        s.push_str(&format!("'l: loop {{\n{fuel_check}{bb}}}\n"));
    } else {
        s.push_str(&format!("{fuel_check}{bb}unreachable!()\n"));
    }
    s.push_str("}\n\n");
    s
}
