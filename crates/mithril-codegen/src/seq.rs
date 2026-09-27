//! Dive-form emission (Core -> sequential Rust) and the expression emitter
//! shared with the rule form. Runtime values are Port raws (`u64`): NUM i56
//! immediates, CON cells (chained past arity 2), boxed floats. Ctor payloads
//! are read via `ctx.cell` (through the generated `field` helper), so no
//! readback is ever needed mid-run.

use mithril_front::ast::{BinOp, CmpOp};
use mithril_front::core::{Core, CoreModule, UNREACHABLE_CTOR};

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

/// Statement-oriented expression emitter. `dive` mode compiles calls as
/// nested `d_*` dives; rule mode must never see a call in expression
/// position (the body is ANF-normalized first).
pub(crate) struct Ex {
    pub tmp: u32,
    pub dive: bool,
    pub self_fid: u32,
    pub loop_form: bool,
}

impl Ex {
    pub fn new(dive: bool, self_fid: u32, loop_form: bool) -> Ex {
        Ex { tmp: 0, dive, self_fid, loop_form }
    }

    pub fn fresh(&mut self) -> String {
        self.tmp += 1;
        format!("t{}", self.tmp)
    }

    /// Emit statements computing `e` into `b`; returns a Rust expression
    /// (temp name, local, or literal) holding the value.
    pub fn val(&mut self, e: &Core, b: &mut String) -> String {
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
            Core::Var(i) => format!("v{i}"),
            Core::Op2(op, x, y) => {
                let ex = self.val(x, b);
                let ey = self.val(y, b);
                let t = self.fresh();
                b.push_str(&format!("let {t} = bin(ctx, {}u8, {ex}, {ey});\n", bin_code(op)));
                t
            }
            Core::Cmp(op, x, y) => {
                let ex = self.val(x, b);
                let ey = self.val(y, b);
                let t = self.fresh();
                b.push_str(&format!("let {t} = cmp(ctx, {}u8, {ex}, {ey});\n", cmp_code(op)));
                t
            }
            Core::If(c, th, el) => {
                let ec = self.val(c, b);
                let bt = self.block_val(th);
                let bf = self.block_val(el);
                let t = self.fresh();
                b.push_str(&format!(
                    "let {t} = if as_i({ec}) != 0 {{\n{bt}}} else {{\n{bf}}};\n"
                ));
                t
            }
            Core::Let(x, r, bo) => {
                let er = self.val(r, b);
                b.push_str(&format!("let v{x} = {er};\n"));
                self.val(bo, b)
            }
            Core::Call(g, args) => {
                assert!(self.dive, "codegen bug: call in a pure rule-form expression");
                let es: Vec<String> = args.iter().map(|a| self.val(a, b)).collect();
                let t = self.fresh();
                b.push_str(&format!("let {t} = d_{g}(ctx, fuel, NONE{})?;\n", commas(&es)));
                t
            }
            Core::Ctor(cid, args) => {
                if *cid == UNREACHABLE_CTOR {
                    let t = self.fresh();
                    b.push_str(&format!("let {t} = mith_unreachable();\n"));
                    return t;
                }
                assert!(*cid < 0xFFE, "codegen: ctor id {} collides with reserved tags", cid);
                assert!(args.len() <= 15, "codegen: ctor arity > 15 unsupported");
                let es: Vec<String> = args.iter().map(|a| self.val(a, b)).collect();
                let t = self.fresh();
                b.push_str(&format!("let {t} = mk_con(ctx, {cid}u16, &[{}]);\n", es.join(", ")));
                t
            }
            Core::Tuple(items) => {
                assert!(items.len() <= 15, "codegen: tuple arity > 15 unsupported");
                let es: Vec<String> = items.iter().map(|a| self.val(a, b)).collect();
                let t = self.fresh();
                b.push_str(&format!("let {t} = mk_con(ctx, 0xFFFu16, &[{}]);\n", es.join(", ")));
                t
            }
            Core::Proj(x, i) => {
                let ex = self.val(x, b);
                let t = self.fresh();
                b.push_str(&format!("let {t} = field(ctx, {ex}, {i});\n"));
                t
            }
            Core::Match(s, arms) => {
                let es = self.val(s, b);
                let t = self.fresh();
                let mut code = format!("let {t} = match con_tag({es}) {{\n");
                for (cid, binders, body) in arms {
                    if *cid == UNREACHABLE_CTOR {
                        continue;
                    }
                    let mut ab = String::new();
                    for (i, bv) in binders.iter().enumerate() {
                        ab.push_str(&format!("let v{bv} = field(ctx, {es}, {i});\n"));
                    }
                    let bb = self.block_val(body);
                    code.push_str(&format!("{cid} => {{\n{ab}{bb}}}\n"));
                }
                code.push_str("_ => mith_unreachable(),\n};\n");
                b.push_str(&code);
                t
            }
        }
    }

    /// `e` as a block body: statements plus a trailing value expression.
    pub fn block_val(&mut self, e: &Core) -> String {
        let mut s = String::new();
        let v = self.val(e, &mut s);
        s.push_str(&v);
        s.push('\n');
        s
    }

    /// Emit `e` in dive tail position: ends every path with `return`, or
    /// `continue 'l` for self tail calls in loop form.
    pub fn dive_tail(&mut self, e: &Core, b: &mut String) {
        match e {
            Core::Let(x, r, bo) => {
                let er = self.val(r, b);
                b.push_str(&format!("let v{x} = {er};\n"));
                self.dive_tail(bo, b);
            }
            Core::If(c, th, el) => {
                let ec = self.val(c, b);
                b.push_str(&format!("if as_i({ec}) != 0 {{\n"));
                self.dive_tail(th, b);
                b.push_str("} else {\n");
                self.dive_tail(el, b);
                b.push_str("}\n");
            }
            Core::Match(s, arms) => {
                let es = self.val(s, b);
                b.push_str(&format!("match con_tag({es}) {{\n"));
                for (cid, binders, body) in arms {
                    if *cid == UNREACHABLE_CTOR {
                        continue;
                    }
                    b.push_str(&format!("{cid} => {{\n"));
                    for (i, bv) in binders.iter().enumerate() {
                        b.push_str(&format!("let v{bv} = field(ctx, {es}, {i});\n"));
                    }
                    self.dive_tail(body, b);
                    b.push_str("}\n");
                }
                b.push_str("_ => { mith_unreachable(); }\n}\n");
            }
            Core::Call(g, args) if *g == self.self_fid && self.loop_form => {
                // Self tail call as a loop iteration: compute all next-state
                // values first (they read the current v*), then assign.
                let es: Vec<String> = args.iter().map(|a| self.val(a, b)).collect();
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
                let es: Vec<String> = args.iter().map(|a| self.val(a, b)).collect();
                b.push_str(&format!("return d_{g}(ctx, fuel, parent{});\n", commas(&es)));
            }
            other => {
                let v = self.val(other, b);
                b.push_str(&format!("return Ok({v});\n"));
            }
        }
    }
}

pub(crate) fn commas(es: &[String]) -> String {
    es.iter().map(|e| format!(", {e}")).collect()
}

/// The dive form of function `fid`: fuel per call / loop iteration; on
/// exhaustion with a known destination the pending call is spawned as a
/// redex (suspension), otherwise the dive unwinds and the caller falls back
/// to the rule form.
pub(crate) fn dive_fn(m: &CoreModule, fid: u32, body: &Core) -> String {
    let f = &m.fns[fid as usize];
    let ar = f.arity;
    let lp = f.self_tail_rec;
    let params: String =
        (0..ar).map(|i| format!(", {}v{}: u64", if lp { "mut " } else { "" }, i)).collect();
    let argl = (0..ar).map(|i| format!("v{i}")).collect::<Vec<_>>().join(", ");
    let fuel_check = format!(
        "*fuel -= 1;\nif *fuel < 0 {{\nif parent != NONE {{\nspawn_call(ctx, {}u16, &[{argl}], parent);\nreturn Err(true);\n}}\nreturn Err(false);\n}}\n",
        1 + fid
    );
    let mut ex = Ex::new(true, fid, lp);
    let mut bb = String::new();
    ex.dive_tail(body, &mut bb);
    let mut s = format!("fn d_{fid}(ctx: &mut Wctx, fuel: &mut i64, parent: u64{params}) -> R {{\n");
    if lp {
        s.push_str(&format!("'l: loop {{\n{fuel_check}{bb}}}\n"));
    } else {
        s.push_str(&format!("{fuel_check}{bb}unreachable!()\n"));
    }
    s.push_str("}\n\n");
    s
}
