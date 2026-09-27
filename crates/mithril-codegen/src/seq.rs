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

use crate::rules::{emit_rec, SegQ};
use crate::ty::{Ty, Types};

/// Values emission shares (dups): their types can never be linear.
#[derive(Default)]
pub(crate) struct Shared {
    pub classes: HashSet<u32>,
    pub tuples: bool,
    pub poison: bool,
}
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
    /// ctor id -> unbox slot (arity-1 int ctors carried in the port).
    pub unbox: &'m std::collections::HashMap<u32, u8>,
    /// fn -> returns a proven i56 (calls to these are int expressions).
    pub iret: &'m [bool],
    /// Inferred types (for share recording) and the module-wide recorder.
    pub tys: &'m Types,
    pub shared: &'m std::cell::RefCell<Shared>,
    /// Pending reuse tokens of the current call-free straight-line span
    /// (dive mode only): consumed cells a following construct may take over.
    pub toks: Vec<String>,
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
            Core::Call(_, a) | Core::Ctor(_, a) | Core::Tuple(a) | Core::Reuse(_, _, a) => {
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
        unbox: &'m std::collections::HashMap<u32, u8>,
        iret: &'m [bool],
        tys: &'m Types,
        shared: &'m std::cell::RefCell<Shared>,
    ) -> Ex<'m> {
        Ex { tmp: 0, dive, self_fid, loop_form, rem, pinned: 0, bset, bor, ints, sq, kframes: Vec::new(), fwd, unbox, iret, tys, shared, toks: Vec::new() }
    }

    /// Release every pending reuse token (before a call, branch, return or
    /// loop back-edge: tokens never cross them, preserving free-early LIFO
    /// locality for whatever runs next).
    pub(crate) fn flush_toks(&mut self, b: &mut String) {
        for t in self.toks.drain(..) {
            b.push_str(&format!("tok_free(ctx, {t});\n"));
        }
    }

    /// Record that variable `i`'s value is shared by emitted code.
    fn note_share(&self, i: u32) {
        let t = if self.self_fid == u32::MAX {
            Ty::Dyn
        } else {
            self.tys.var(self.self_fid as usize, i)
        };
        if std::env::var_os("MITHRIL_DEBUG_SHARE").is_some() {
            eprintln!("share: fn {} var {} ty {:?} pinned {} rem {:?}", self.self_fid, i, t, self.pinned, self.rem.get(&i));
        }
        let mut sh = self.shared.borrow_mut();
        match t {
            Ty::Int => {}
            Ty::Flo => {} // boxed floats always carry an rc
            Ty::Adt(c) => {
                sh.classes.insert(c);
            }
            Ty::Tup(_) => sh.tuples = true,
            Ty::Dyn => sh.poison = true,
        }
    }

    /// Emit the suspension path for a dive call whose result `rv` (a record
    /// awaiting its parent) came back as `Err`: build the continuation
    /// record chain — `own` (this let's continuation) then every enclosing
    /// value-position frame — patch parents inward, and return the
    /// outermost record for the caller to attach.
    pub(crate) fn emit_capture(&mut self, rv: &str, own: Option<(u32, &Core)>, b: &mut String) {
        let mut cb = String::new();
        let child = self.capture_chain(rv, own, &mut cb);
        cb.push_str(&format!("{child} as u64\n"));
        // The chain runs once per suspension; keeping it out of line keeps
        // its record/spawn temporaries (and their stack slots) out of the
        // hot function so the entry fuel test can shrink-wrap.
        let name = format!("cap_{}", self.fresh());
        let vars = outer_vars(&cb);
        let params: String = vars.iter().map(|v| format!(", {v}: u64")).collect();
        b.push_str(&format!(
            "#[cold] #[inline(never)] fn {name}(ctx: &mut Wctx, {rv}: u32{params}) -> u64 {{\n{cb}}}\nreturn Err({name}(ctx, {rv} as u32{}));\n",
            vars.iter().map(|v| format!(", {v}")).collect::<String>()
        ));
    }

    /// Emit the record chain for a suspension into `b`, returning the name
    /// of the outermost record (see `emit_capture`).
    fn capture_chain(&mut self, rv: &str, own: Option<(u32, &Core)>, b: &mut String) -> String {
        let saved = self.rem.clone();
        let mut frames: Vec<(u32, Core)> = Vec::new();
        if let Some((x, bo)) = own {
            frames.push((x, bo.clone()));
        }
        for f in self.kframes.iter().rev() {
            frames.push(f.clone());
        }
        let mut child = rv.to_string();
        // Each frame's continuation splits into P, the bindings that do not
        // (transitively) need the pending value x, and J, the rest plus the
        // tail. When P does real work it has not started yet — spawn it now
        // as a task behind a pend-2 join record so it runs in parallel with
        // the suspended x, exactly like the rule form's pair fork but for
        // the whole independent suffix (`fa = f(x); y = g(..); fb = f(y)`
        // must not wait for fa). Sequential order is untouched: this only
        // happens on fuel-out.
        for (x, bo) in &frames {
            if let Some((p_body, live, j_body)) = split_frame(*x, bo) {
                let mut env_j = free_vars(&j_body);
                env_j.remove(x);
                env_j.remove(&live);
                let env_j: Vec<u32> = env_j.into_iter().collect();
                let sid_j = self.sq.as_mut().expect("dive capture without a segment registry").add(self.self_fid, vec![*x, live], env_j.clone(), j_body.clone());
                let env_p: Vec<u32> = free_vars(&p_body).into_iter().collect();
                let sid_p = self.sq.as_mut().expect("dive capture without a segment registry").add(self.self_fid, vec![], env_p.clone(), p_body.clone());
                let rn = emit_rec(self, &env_j, sid_j, 2, "NONE", &j_body, b);
                b.push_str(&format!("ctx.set_parent({child}, ({rn} as u64) << 3);\n"));
                let rp = emit_rec(self, &env_p, sid_p, 0, &format!("((({rn} as u64) << 3) | 1)"), &p_body, b);
                b.push_str(&format!("ctx.ready_rec({rp});\n"));
                child = rn;
            } else {
                let mut env = free_vars(bo);
                env.remove(x);
                let env: Vec<u32> = env.into_iter().collect();
                let sid = self.sq.as_mut().expect("dive capture without a segment registry").add(self.self_fid, vec![*x], env.clone(), bo.clone());
                let rn = emit_rec(self, &env, sid, 1, "NONE", bo, b);
                b.push_str(&format!("ctx.set_parent({child}, ({rn} as u64) << 3);\n"));
                child = rn;
            }
        }
        self.rem = saved;
        child
    }

    /// A let whose RHS is a call: run the dive, capturing on suspension.
    fn let_call(&mut self, x: u32, g: u32, args: &[Core], bo: &Core, b: &mut String) {
        let (call, post) = self.dive_call(g, args, b);
        let t = self.fresh();
        b.push_str(&format!(
            "let {t} = match {call} {{\nOk(v) => v,\nErr(r) => {{\n"
        ));
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
            Core::Num(_) | Core::Cmp(..) => true,
            Core::Op2(_, a, b) => self.is_int(a) && self.is_int(b),
            Core::Var(i) => self.ints.contains(i),
            Core::Call(g, _) => self.iret.get(*g as usize).copied().unwrap_or(false),
            Core::If(_, t, f) => self.is_int(t) && self.is_int(f),
            Core::Let(_, _, b) => self.is_int(b),
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
                self.note_share(i);
                let t = self.fresh();
                b.push_str(&format!("let {t} = dup_val(ctx, v{i});\n"));
                return t;
            }
            return format!("v{i}");
        }
        if self.pinned > 0 {
            self.note_share(i);
            let t = self.fresh();
            b.push_str(&format!("let {t} = dup_val(ctx, v{i});\n"));
            return t;
        }
        let r0 = self.rem.get(&i).copied();
        if r0.map_or(true, |r| r >= 2) {
            self.note_share(i);
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

    /// Read variable `i` as an escaping value standing for `n` of its
    /// remaining uses at once (a record env slot serves every use the
    /// segment body makes): the last of them moves, earlier ones share.
    pub fn use_var_n(&mut self, i: u32, n: i64, b: &mut String) -> String {
        if n > 1 {
            if let Some(r) = self.rem.get_mut(&i) {
                *r = (*r - (n - 1)).max(1);
            }
        }
        self.use_var(i, true, b)
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
                self.note_share(*i);
                return (format!("v{i}"), Hold::BorrowDup);
            }
            if self.rem.get(i).copied().map_or(true, |r| r >= 2) {
                self.note_share(*i);
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
        cid: u32,
        binders: &[u32],
        tok: Option<u32>,
        b: &mut String,
    ) {
        // Consume with a reuse token (the rewrite placed a `Reuse` of this
        // scrutinee var on some path of the arm)
        if self.dive && hold == Hold::Consume && binders.len() == 2 && tok.is_some() {
            let names: Vec<String> = binders.iter().map(|bv| format!("v{bv}")).collect();
            let tk = format!("tok_v{}", tok.unwrap());
            b.push_str(&format!(
                "let ({}, {}, {tk}) = consume2r(ctx, {sv}, {cid}u16);\n",
                names[0], names[1]
            ));
            self.toks.push(tk);
            for bv in binders {
                if self.rem.get(bv).copied().unwrap_or(0) == 0 {
                    b.push_str(&format!("free_val(ctx, v{bv});\n"));
                }
            }
            return;
        }
        if hold == Hold::Consume && !binders.is_empty() && binders.len() <= 2 {
            let names: Vec<String> = binders.iter().map(|bv| format!("v{bv}")).collect();
            match binders.len() {
                1 => b.push_str(&format!(
                    "let ({}, m_unused) = consume2k(ctx, {sv}, {cid}u16);\nfree_val(ctx, m_unused);\n",
                    names[0]
                )),
                _ => b.push_str(&format!("let ({}, {}) = consume2k(ctx, {sv}, {cid}u16);\n", names[0], names[1])),
            }
            for bv in binders {
                if self.rem.get(bv).copied().unwrap_or(0) == 0 {
                    b.push_str(&format!("free_val(ctx, v{bv});\n"));
                }
            }
            return;
        }
        for (i, bv) in binders.iter().enumerate() {
            let cnt = self.rem.get(bv).copied().unwrap_or(0);
            match hold {
                Hold::BorrowRaw => {
                    b.push_str(&format!("let v{bv} = field(ctx, {sv}, {i});\n"));
                }
                Hold::Consume => {
                    // chained arity: incref used fields, decref the root
                    if cnt > 0 {
                        b.push_str(&format!(
                            "let v{bv} = dup_val(ctx, field(ctx, {sv}, {i}));\n"
                        ));
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
            b.push_str(&format!("free_val(ctx, {sv});\n"));
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
        // args first (they may take tokens), then release the rest (below)
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
        self.flush_toks(b);
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
                self.flush_toks(b);
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
                if let Some(slot) = self.unbox.get(cid) {
                    let e0 = self.val(&args[0], false, b);
                    let t = self.fresh();
                    b.push_str(&format!("let {t} = ic({slot}u64, as_i({e0}));\n"));
                    return t;
                }
                let es: Vec<String> = args.iter().map(|a| self.val(a, true, b)).collect();
                let t = self.fresh();
                b.push_str(&mk_con_call(&t, *cid as u32, &es));
                t
            }
            Core::Reuse(v, cid, args) => {
                // decided by the reuse rewrite: build in v's consumed cell
                let es: Vec<String> = args.iter().map(|a| self.val(a, true, b)).collect();
                let t = self.fresh();
                let tk = format!("tok_v{v}");
                if self.dive && self.toks.iter().any(|x| *x == tk) {
                    self.toks.retain(|x| *x != tk);
                    b.push_str(&format!("let {t} = mk_con2r(ctx, {tk}, {}u16, {}, {});\n", *cid, es[0], es[1]));
                } else {
                    b.push_str(&mk_con_call(&t, *cid as u32, &es));
                }
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
                b.push_str(&format!("let {t} = dup_val(ctx, field(ctx, {sv}, {i}));\n"));
                if hold == Hold::Consume {
                    b.push_str(&format!("free_val(ctx, {sv});\n"));
                }
                t
            }
            Core::Match(s, arms) => {
                let (sv, hold) = self.scrutinee(s, b);
                self.flush_toks(b);
                let t = self.fresh();
                let (open, plan, close) = plan_arms(&sv, arms, self.unbox);
                let mut code = format!("let {t} = {open}");
                self.pinned += 1;
                for (i, pre, suf) in plan {
                    let (cid, binders, body) = &arms[i];
                    let mut ab = String::new();
                    if self.unbox.contains_key(cid) {
                        if let Some(bv) = binders.first() {
                            if self.rem.get(bv).copied().unwrap_or(0) > 0 || self.pinned > 0 {
                                ab.push_str(&format!("let v{bv} = num(as_i({sv}));\n"));
                            }
                        }
                    } else {
                        let tok = reuse_var(body, &sv);
                        self.bind_fields(&sv, hold, *cid, binders, tok, &mut ab);
                    }
                    let bb = self.block_val(body, esc);
                    code.push_str(&format!("{pre}{ab}{bb}{suf}"));
                }
                self.pinned -= 1;
                code.push_str(&format!("{close};\n"));
                b.push_str(&code);
                t
            }
        }
    }

    /// `e` as a block body: statements plus a trailing value expression.
    pub fn block_val(&mut self, e: &Core, esc: bool) -> String {
        let outer = std::mem::take(&mut self.toks);
        let mut s = String::new();
        let v = self.val(e, esc, &mut s);
        self.flush_toks(&mut s);
        self.toks = outer;
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
                // tail branches inherit pending tokens; every arm ends in a
                // terminal (call/return/back-edge) that releases its unused ones
                let toks = self.toks.clone();
                let saved = self.rem.clone();
                b.push_str(&format!("if as_i({ec}) != 0 {{\n"));
                self.dive_tail(th, b);
                self.rem = saved.clone();
                self.toks = toks.clone();
                b.push_str("} else {\n");
                self.dive_tail(el, b);
                self.rem = saved;
                self.toks.clear();
                b.push_str("}\n");
            }
            Core::Match(s, arms) => {
                let (sv, hold) = self.scrutinee(s, b);
                let toks = self.toks.clone();
                let saved = self.rem.clone();
                let (open, plan, close) = plan_arms(&sv, arms, self.unbox);
                b.push_str(&open);
                for (i, pre, suf) in plan {
                    let (cid, binders, body) = &arms[i];
                    self.rem = saved.clone();
                    self.toks = toks.clone();
                    b.push_str(&pre);
                    if self.unbox.contains_key(cid) {
                        if let Some(bv) = binders.first() {
                            if self.rem.get(bv).copied().unwrap_or(0) > 0 || self.pinned > 0 {
                                b.push_str(&format!("let v{bv} = num(as_i({sv}));\n"));
                            }
                        }
                    } else {
                        let tok = reuse_var(body, &sv);
                        self.bind_fields(&sv, hold, *cid, binders, tok, b);
                    }
                    self.dive_tail(body, b);
                    b.push_str(&suf);
                }
                self.rem = saved;
                self.toks.clear();
                b.push_str(&format!("{close}\n"));
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
                self.flush_toks(b);
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
                self.flush_toks(b);
                b.push_str(&format!("return Ok({v});\n"));
            }
        }
    }
}

/// The scrutinee variable (if `sv` names one) when `body` reuses its cell.
pub(crate) fn reuse_var(body: &Core, sv: &str) -> Option<u32> {
    let v: u32 = sv.strip_prefix('v')?.parse().ok()?;
    fn has(e: &Core, v: u32) -> bool {
        match e {
            Core::Reuse(w, _, xs) => *w == v || xs.iter().any(|x| has(x, v)),
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => false,
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => has(a, v) || has(b, v),
            Core::If(c, t, f) => has(c, v) || has(t, v) || has(f, v),
            Core::Let(_, r, b) => has(r, v) || has(b, v),
            Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) => xs.iter().any(|x| has(x, v)),
            Core::Proj(b, _) => has(b, v),
            Core::Match(s, arms) => has(s, v) || arms.iter().any(|(_, _, b)| has(b, v)),
        }
    }
    if has(body, v) { Some(v) } else { None }
}

/// Switch lowering for a constructor match on `sv`: dispatch on the port's
/// tag byte first (an unboxed ctor IS its tag byte), then on `con_tag` only
/// among >= 2 boxed ctors; the last boxed arm takes the remaining case
/// (matches are exhaustive by construction). Returns the opening text, the
/// arms in emission order as (index into `arms`, prefix, suffix), and the
/// closing text (without a trailing `;`).
pub(crate) fn plan_arms(
    sv: &str,
    arms: &[(u32, Vec<u32>, Core)],
    unbox: &std::collections::HashMap<u32, u8>,
) -> (String, Vec<(usize, String, String)>, String) {
    let live: Vec<usize> = (0..arms.len()).filter(|&i| arms[i].0 != UNREACHABLE_CTOR).collect();
    let ub: Vec<usize> = live.iter().copied().filter(|&i| unbox.contains_key(&arms[i].0)).collect();
    let bx: Vec<usize> = live.iter().copied().filter(|&i| !unbox.contains_key(&arms[i].0)).collect();
    let mut plan = Vec::new();
    for &i in &ub {
        let slot = unbox[&arms[i].0] as u64;
        plan.push((i, format!("{} => {{\n", 16 + slot), "}\n".to_string()));
    }
    let close;
    match bx.len() {
        0 => close = "_ => mith_unreachable(),\n}".to_string(),
        1 => {
            plan.push((bx[0], "_ => {\n".to_string(), "}\n".to_string()));
            close = "}".to_string();
        }
        n => {
            for (j, &i) in bx.iter().enumerate() {
                let pre = if j == 0 {
                    format!("_ => match con_tag({sv}) {{\n{} => {{\n", arms[i].0)
                } else if j == n - 1 {
                    "_ => {\n".to_string()
                } else {
                    format!("{} => {{\n", arms[i].0)
                };
                let suf = if j == n - 1 { "}\n}\n".to_string() } else { "}\n".to_string() };
                plan.push((i, pre, suf));
            }
            close = "}".to_string();
        }
    }
    (format!("match tag({sv}) {{\n"), plan, close)
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
    unbox: &'m std::collections::HashMap<u32, u8>,
    tys: &'m Types,
    iret: &'m [bool],
    shared: &'m std::cell::RefCell<Shared>,
) -> String {
    let f = &m.fns[fid as usize];
    let ar = f.arity;
    let lp = f.self_tail_rec;
    let params: String =
        (0..ar).map(|i| format!(", {}v{}: u64", if lp { "mut " } else { "" }, i)).collect();
    let argl = (0..ar).map(|i| format!("v{i}")).collect::<Vec<_>>().join(", ");
    // Fuel-out at entry / loop top: the pending call IS the continuation;
    // spawn it against a forwarding record whose parent the caller sets.
    let cparams: String = (0..ar).map(|i| format!(", v{i}: u64")).collect();
    let fuel_check = format!(
        "*fuel -= 1;\nif *fuel < 0 {{\n#[cold] #[inline(never)] fn cap(ctx: &mut Wctx{cparams}) -> u64 {{\nlet r = ctx.alloc_rec({fwd}u16, 1, 0, 0, NONE);\nspawn_call(ctx, {}u16, &[{argl}], (r as u64) << 3);\nr as u64\n}}\nreturn Err(cap(ctx{}));\n}}\n",
        1 + fid,
        (0..ar).map(|i| format!(", v{i}")).collect::<String>()
    );
    let mut rem = Cnt::new();
    cnt_dive(body, &mut rem);
    let ints = crate::ints_of(tys, fid as usize);
    let mut ex = Ex::new(true, fid, lp, rem, bset.clone(), bor, ints, Some(sq), fwd, unbox, iret, tys, shared);
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
    // A call-free body does bounded work: no fuel check, and it inlines
    // into its (recursive) callers.
    let leafy = !has_call(body);
    let mut s = format!(
        "{}#[allow(clippy::too_many_arguments)]\nfn d_{fid}(ctx: &mut Wctx, fuel: &mut i64{params}) -> R {{\n",
        if leafy { "" } else { "" }
    );
    if lp {
        let fc = if leafy { String::new() } else { fuel_check.clone() };
        s.push_str(&format!("'l: loop {{\n{fc}{bb}}}\n"));
    } else if leafy {
        s.push_str(&format!("let _ = fuel;\n{bb}unreachable!()\n"));
    } else {
        s.push_str(&format!("{fuel_check}{bb}unreachable!()\n"));
    }
    s.push_str("}\n\n");
    s
}

/// The `v<n>` variables a generated block reads from its enclosing scope:
/// every `v<n>` identifier it mentions that it does not itself `let`-bind.
pub(crate) fn outer_vars(code: &str) -> Vec<String> {
    let bytes = code.as_bytes();
    let mut used: Vec<String> = Vec::new();
    let mut bound: HashSet<String> = HashSet::new();
    let mut i = 0;
    let mut after_let = false;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_alphabetic() || c == '_' {
            let st = i;
            while i < bytes.len() && ((bytes[i] as char).is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let id = &code[st..i];
            if id == "let" {
                after_let = true;
                continue;
            }
            if id == "mut" && after_let {
                continue;
            }
            if after_let {
                // everything up to the `=` is the pattern (tuples included)
                bound.insert(id.to_string());
            } else if id.len() > 1 && id.starts_with('v') && id[1..].bytes().all(|d| d.is_ascii_digit()) {
                if !used.iter().any(|u| u == id) {
                    used.push(id.to_string());
                }
            }
        } else {
            if c == '=' || c == ';' {
                after_let = false;
            }
            i += 1;
        }
    }
    used.retain(|u| !bound.contains(u));
    used
}

/// Split a suspended frame's continuation `bo` (a let chain) around the
/// pending value `x`: `Some((P, l, J))` where P is the chain of bindings
/// independent of x ending in its one live-out `l` (the single binder J
/// reads), and J is the dependent rest. `None` when P does no call or
/// has zero or several live-outs (those frames wait as plain records).
pub(crate) fn split_frame(x: u32, bo: &Core) -> Option<(Core, u32, Core)> {
    let mut binds: Vec<(u32, &Core)> = Vec::new();
    let mut cur = bo;
    while let Core::Let(v, r, b) = cur {
        binds.push((*v, r));
        cur = b;
    }
    let mut dep: HashSet<u32> = HashSet::from([x]);
    let (mut p, mut j): (Vec<(u32, &Core)>, Vec<(u32, &Core)>) = (Vec::new(), Vec::new());
    for (v, r) in binds {
        if free_vars(r).iter().any(|f| dep.contains(f)) {
            dep.insert(v);
            j.push((v, r));
        } else {
            p.push((v, r));
        }
    }
    if !p.iter().any(|(_, r)| has_call(r)) {
        return None;
    }
    let mut j_body = cur.clone();
    for (v, r) in j.iter().rev() {
        j_body = Core::Let(*v, Box::new((*r).clone()), Box::new(j_body));
    }
    let jf = free_vars(&j_body);
    let live: Vec<u32> = p.iter().map(|(v, _)| *v).filter(|v| jf.contains(v)).collect();
    if live.len() != 1 {
        return None;
    }
    let mut p_body = Core::Var(live[0]);
    for (v, r) in p.iter().rev() {
        p_body = Core::Let(*v, Box::new((*r).clone()), Box::new(p_body));
    }
    Some((p_body, live[0], j_body))
}
