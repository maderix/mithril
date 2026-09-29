//! The lowered IR every emitter builds and every backend prints.
//!
//! One lowering, several printers: the decisions (ownership, fuel,
//! suspension, representation) are made once by the emitters in terms of
//! these statements and a fixed helper vocabulary (the `mithril_rt::prelude`
//! names plus the generated program's own helpers); a backend only prints
//! them. `rust::func` is the CPU backend.
//!
//! Values are locals with a declared type; control is `If` / `Switch` /
//! `Loop` at statement level (value-position branches are lowered to a
//! declaration plus assignments, which every target has). A call that may
//! suspend is `Try`: its `Ok` value binds the pattern, its `Err` payload
//! (a record) runs the handler, which must leave the function. A `Res`
//! match handles both outcomes in place.

use std::fmt::Write;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ty {
    U64,
    I64,
    U32,
    U16,
    U8,
    Bool,
    Usize,
    /// `Result<u64, u64>`: a value or a suspension record.
    Res,
    /// `Result<[u64; k], u64>`: native multi-value dive result.
    ResArr(usize),
    /// `[u64; k]`
    Arr(usize),
    /// `(i64, .., i64)` of `k`
    Tup(usize),
    /// `&mut u64` / `&mut u32` / `&mut i64` out-parameters
    RefU64,
    RefU32,
    RefI64,
    /// the type the initializer has (destructuring lets)
    Infer,
    Unit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bop {
    Add,
    Sub,
    Mul,
    Div,
    Shl,
    Shr,
    And,
    Or,
    Xor,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

impl Bop {
    /// The operator symbol of a non-wrapping (bit or comparison) op.
    pub fn sym(self) -> &'static str {
        match self {
            Bop::And => "&",
            Bop::Or => "|",
            Bop::Xor => "^",
            Bop::Lt => "<",
            Bop::Le => "<=",
            Bop::Gt => ">",
            Bop::Ge => ">=",
            Bop::Eq => "==",
            Bop::Ne => "!=",
            _ => unreachable!("wrapping op {self:?} has no symbol"),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum E {
    /// integer literal of a type
    Int(i64, Ty),
    Bool(bool),
    /// an f64 literal (by bit pattern)
    Flo(f64),
    /// a local
    V(String),
    /// `*p` of an out-parameter
    Deref(String),
    /// `&mut x`
    Ref(String),
    /// `&x`
    Addr(String),
    /// helper or generated function call; `ctx` = takes the worker context
    Call { f: String, ctx: bool, args: Vec<E> },
    /// wrapping arithmetic / bit op / comparison on same-typed operands
    Bin(Bop, Box<E>, Box<E>),
    Not(Box<E>),
    Neg(Box<E>),
    Cast(Box<E>, Ty),
    /// `x[i]` of an array value
    Idx(Box<E>, usize),
    Tup(Vec<E>),
    Arr(Vec<E>),
    /// `&[..]`: a slice argument
    Slice(Vec<E>),
    Ok(Box<E>),
    Err(Box<E>),
    /// a program constant (`NONE`, `NOHOLE`, a `FOLD_EST_n` atomic ..)
    Const(String),
}

#[derive(Clone, PartialEq, Debug)]
pub enum Pat {
    One(String),
    Tup(Vec<String>),
    Arr(Vec<String>),
}

#[derive(Clone, PartialEq, Debug)]
pub enum S {
    /// `let pat: ty = e;`
    Let(Pat, Ty, E),
    /// `let x: ty;` (assigned on every path before use)
    Decl(String, Ty),
    Set(String, E),
    /// `*p = e`
    Store(String, E),
    Do(E),
    If(E, Vec<S>, Vec<S>),
    /// integer switch with a default (`unreachable` when `None`)
    Switch(E, Vec<(u64, Vec<S>)>, Option<Vec<S>>),
    Loop(Vec<S>),
    Continue,
    Ret(E),
    /// `let pat = match e { Ok(v) => v, Err(r) => { handler } }`; the
    /// handler leaves the function
    Try(Pat, E, String, Vec<S>),
    /// `match e { Ok(v) => { .. }, Err(r) => { .. } }`
    Res(E, String, Vec<S>, String, Vec<S>),
    /// a nested (cold, out-of-line) function
    Fn(Box<FnDef>),
    /// diverges (`unreachable!()`)
    Unreachable,
    Comment(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Inline {
    Default,
    Always,
    Never,
}

#[derive(Clone, PartialEq, Debug)]
pub struct FnDef {
    pub name: String,
    /// takes the worker context (`ctx`) first
    pub ctx: bool,
    pub params: Vec<(String, Ty)>,
    pub ret: Ty,
    pub body: Vec<S>,
    pub inline: Inline,
    pub cold: bool,
}

// ---- constructors ----

pub fn v(s: impl Into<String>) -> E {
    E::V(s.into())
}
pub fn i64_(n: i64) -> E {
    E::Int(n, Ty::I64)
}
pub fn u64_(n: u64) -> E {
    E::Int(n as i64, Ty::U64)
}
pub fn u16_(n: u64) -> E {
    E::Int(n as i64, Ty::U16)
}
pub fn u8_(n: u64) -> E {
    E::Int(n as i64, Ty::U8)
}
pub fn u32_(n: u64) -> E {
    E::Int(n as i64, Ty::U32)
}
pub fn usize_(n: usize) -> E {
    E::Int(n as i64, Ty::Usize)
}
/// A context-taking call.
pub fn c(f: &str, args: Vec<E>) -> E {
    E::Call { f: f.to_string(), ctx: true, args }
}
/// A pure call.
pub fn p(f: &str, args: Vec<E>) -> E {
    E::Call { f: f.to_string(), ctx: false, args }
}
pub fn bin(op: Bop, a: E, b: E) -> E {
    E::Bin(op, Box::new(a), Box::new(b))
}
pub fn cast(e: E, t: Ty) -> E {
    E::Cast(Box::new(e), t)
}
pub fn idx(e: E, i: usize) -> E {
    E::Idx(Box::new(e), i)
}
pub fn ok(e: E) -> E {
    E::Ok(Box::new(e))
}
pub fn err(e: E) -> E {
    E::Err(Box::new(e))
}
pub fn let_(x: impl Into<String>, t: Ty, e: E) -> S {
    S::Let(Pat::One(x.into()), t, e)
}
pub fn set(x: impl Into<String>, e: E) -> S {
    S::Set(x.into(), e)
}
pub fn do_(e: E) -> S {
    S::Do(e)
}
pub fn ret(e: E) -> S {
    S::Ret(e)
}
/// `return;`
pub fn ret_unit() -> S {
    S::Ret(E::Tup(vec![]))
}
pub fn as_i(e: E) -> E {
    p("as_i", vec![e])
}
pub fn num(e: E) -> E {
    p("num", vec![e])
}
pub fn free(e: E) -> S {
    do_(c("free_val", vec![e]))
}
/// `as_i(e) != 0`
pub fn truthy(e: E) -> E {
    bin(Bop::Ne, as_i(e), i64_(0))
}
/// `(rec as u64) << 3`: the record address of a record index
pub fn rec_addr(rn: &str) -> E {
    p("rec_addr", vec![v(rn)])
}
/// `*fuel -= 1`
pub fn burn_fuel() -> S {
    S::Store("fuel".into(), bin(Bop::Sub, E::Deref("fuel".into()), i64_(1)))
}
/// `n` units of work charged to the budget (loop iterations, native leaf
/// calls): the CPU burns them (its suspension splits work); the device
/// does not (its budget bounds stack depth only; a loop runs to its end).
pub fn work_fuel(n: E) -> S {
    do_(p("work_fuel", vec![v("fuel"), n]))
}

// ---- analyses ----

pub fn pat_names(p: &Pat) -> Vec<String> {
    match p {
        Pat::One(x) => vec![x.clone()],
        Pat::Tup(xs) | Pat::Arr(xs) => xs.clone(),
    }
}

impl E {
    /// Pre-order visit of every sub-expression.
    pub fn walk(&self, f: &mut dyn FnMut(&E)) {
        f(self);
        match self {
            E::Call { args, .. } | E::Tup(args) | E::Arr(args) | E::Slice(args) => args.iter().for_each(|a| a.walk(f)),
            E::Bin(_, a, b) => {
                a.walk(f);
                b.walk(f)
            }
            E::Not(a) | E::Neg(a) | E::Cast(a, _) | E::Idx(a, _) | E::Ok(a) | E::Err(a) => a.walk(f),
            E::Int(..) | E::Bool(_) | E::Flo(_) | E::V(_) | E::Deref(_) | E::Ref(_) | E::Addr(_) | E::Const(_) => {}
        }
    }
}

impl S {
    /// A statement's own expression and its nested blocks, in execution
    /// order (a nested `Fn` is neither).
    fn parts(&self) -> (Option<&E>, Vec<&[S]>) {
        match self {
            S::Let(_, _, e) | S::Set(_, e) | S::Store(_, e) | S::Do(e) | S::Ret(e) => (Some(e), vec![]),
            S::If(e, a, b) | S::Res(e, _, a, _, b) => (Some(e), vec![&a[..], &b[..]]),
            S::Switch(e, arms, d) => (Some(e), arms.iter().map(|(_, b)| &b[..]).chain(d.as_deref()).collect()),
            S::Try(_, e, _, h) => (Some(e), vec![&h[..]]),
            S::Loop(b) => (None, vec![&b[..]]),
            S::Decl(..) | S::Continue | S::Fn(_) | S::Unreachable | S::Comment(_) => (None, vec![]),
        }
    }
}

/// Locals a statement list reads that it does not itself bind (name
/// filter `f`): the parameters an out-of-line copy of it needs, in first-read
/// order (a nested `cap_*` signature follows it).
pub fn free_locals(body: &[S], f: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut used = Vec::new();
    let mut bound = std::collections::HashSet::new();
    fn st(s: &S, used: &mut Vec<String>, bound: &mut std::collections::HashSet<String>, f: &dyn Fn(&str) -> bool) {
        if let S::Fn(d) = s {
            for x in free_locals(&d.body, f) {
                if !d.params.iter().any(|(pn, _)| *pn == x) && !used.contains(&x) {
                    used.push(x);
                }
            }
            return;
        }
        let (e, bs) = s.parts();
        if let Some(e) = e {
            e.walk(&mut |e| {
                if let E::V(x) | E::Deref(x) | E::Ref(x) | E::Addr(x) = e {
                    if f(x) && !used.contains(x) {
                        used.push(x.clone());
                    }
                }
            });
        }
        match s {
            S::Decl(x, _) | S::Try(_, _, x, _) => {
                bound.insert(x.clone());
            }
            S::Res(_, o, _, r, _) => bound.extend([o.clone(), r.clone()]),
            S::Set(x, _) | S::Store(x, _) if f(x) && !used.contains(x) && !bound.contains(x) => used.push(x.clone()),
            _ => {}
        }
        bs.into_iter().flatten().for_each(|s| st(s, used, bound, f));
        if let S::Let(pt, ..) | S::Try(pt, ..) = s {
            bound.extend(pat_names(pt));
        }
    }
    body.iter().for_each(|s| st(s, &mut used, &mut bound, f));
    used.retain(|u| !bound.contains(u));
    used
}

/// Locals assigned or mutably borrowed after their binding.
fn assigned(body: &[S], out: &mut std::collections::HashSet<String>) {
    for s in body {
        if let S::Set(x, _) = s {
            out.insert(x.clone());
        }
        let (e, bs) = s.parts();
        if let Some(e) = e {
            e.walk(&mut |e| {
                if let E::Ref(x) = e {
                    out.insert(x.clone());
                }
            });
        }
        bs.into_iter().for_each(|b| assigned(b, out));
    }
}

// ---- the Rust printer ----

pub mod rust {
    use super::*;

    fn ty(t: Ty) -> String {
        match t {
            Ty::U64 => "u64".into(),
            Ty::I64 => "i64".into(),
            Ty::U32 => "u32".into(),
            Ty::U16 => "u16".into(),
            Ty::U8 => "u8".into(),
            Ty::Bool => "bool".into(),
            Ty::Usize => "usize".into(),
            Ty::Res => "R".into(),
            Ty::ResArr(k) => format!("Result<[u64; {k}], u64>"),
            Ty::Arr(k) => format!("[u64; {k}]"),
            Ty::Tup(k) => format!("({})", vec!["i64"; k].join(", ")),
            Ty::RefU64 => "&mut u64".into(),
            Ty::RefU32 => "&mut u32".into(),
            Ty::RefI64 => "&mut i64".into(),
            Ty::Infer => "_".into(),
            Ty::Unit => "()".into(),
        }
    }

    pub fn ex(e: &E) -> String {
        match e {
            E::Int(n, t) => match t {
                Ty::U64 => format!("{}u64", *n as u64),
                Ty::I64 | Ty::U32 | Ty::U16 | Ty::U8 | Ty::Usize => format!("{n}{}", ty(*t)),
                _ => format!("{n}"),
            },
            E::Bool(b) => b.to_string(),
            E::Flo(x) => format!("f64::from_bits(0x{:016x}u64)", x.to_bits()),
            E::V(x) | E::Const(x) => x.clone(),
            E::Deref(x) => format!("(*{x})"),
            E::Ref(x) => format!("&mut {x}"),
            E::Addr(x) => format!("&{x}"),
            E::Call { f, ctx, args } => {
                let mut a: Vec<String> = Vec::new();
                if *ctx {
                    a.push("ctx".into());
                }
                a.extend(args.iter().map(ex));
                format!("{f}({})", a.join(", "))
            }
            E::Bin(op, a, b) => {
                let (a, b) = (ex(a), ex(b));
                let w = |m: &str| format!("{a}.wrapping_{m}({b})");
                match op {
                    Bop::Add => w("add"),
                    Bop::Sub => w("sub"),
                    Bop::Mul => w("mul"),
                    Bop::Div => w("div"),
                    Bop::Shl => format!("{a}.wrapping_shl({b} as u32)"),
                    Bop::Shr => format!("{a}.wrapping_shr({b} as u32)"),
                    _ => format!("({a} {} {b})", op.sym()),
                }
            }
            E::Not(a) => format!("!{}", ex(a)),
            E::Neg(a) => format!("(-({}))", ex(a)),
            E::Cast(a, t) => format!("({} as {})", ex(a), ty(*t)),
            E::Idx(a, i) => format!("{}[{i}]", ex(a)),
            E::Tup(xs) => format!("({})", xs.iter().map(ex).collect::<Vec<_>>().join(", ")),
            E::Arr(xs) => format!("[{}]", xs.iter().map(ex).collect::<Vec<_>>().join(", ")),
            E::Slice(xs) => format!("&[{}]", xs.iter().map(ex).collect::<Vec<_>>().join(", ")),
            E::Ok(a) => format!("Ok({})", ex(a)),
            E::Err(a) => format!("Err({})", ex(a)),
        }
    }

    fn pat(p: &Pat, muts: &std::collections::HashSet<String>) -> String {
        let m = |x: &String| if muts.contains(x) { format!("mut {x}") } else { x.clone() };
        match p {
            Pat::One(x) => m(x),
            Pat::Tup(xs) => format!("({})", xs.iter().map(m).collect::<Vec<_>>().join(", ")),
            Pat::Arr(xs) => format!("[{}]", xs.iter().map(m).collect::<Vec<_>>().join(", ")),
        }
    }

    pub fn stmts(body: &[S], muts: &std::collections::HashSet<String>, out: &mut String) {
        for s in body {
            match s {
                S::Let(pt, t, e) => {
                    let ann = if *t == Ty::Infer { String::new() } else { format!(": {}", ty(*t)) };
                    let _ = writeln!(out, "let {}{ann} = {};", pat(pt, muts), ex(e));
                }
                S::Decl(x, t) => {
                    let _ = writeln!(out, "let {}: {};", pat(&Pat::One(x.clone()), muts), ty(*t));
                }
                S::Set(x, e) => {
                    let _ = writeln!(out, "{x} = {};", ex(e));
                }
                S::Store(x, e) => {
                    let _ = writeln!(out, "*{x} = {};", ex(e));
                }
                S::Do(e) => {
                    let _ = writeln!(out, "{};", ex(e));
                }
                S::If(cnd, a, b) => {
                    let _ = writeln!(out, "if {} {{", ex(cnd));
                    stmts(a, muts, out);
                    if !b.is_empty() {
                        out.push_str("} else {\n");
                        stmts(b, muts, out);
                    }
                    out.push_str("}\n");
                }
                S::Switch(e, arms, d) => {
                    let _ = writeln!(out, "match {} {{", ex(e));
                    for (k, b) in arms {
                        let _ = writeln!(out, "{k} => {{");
                        stmts(b, muts, out);
                        out.push_str("}\n");
                    }
                    out.push_str("_ => {\n");
                    match d {
                        Some(b) => stmts(b, muts, out),
                        None => out.push_str("mith_unreachable();\n"),
                    }
                    out.push_str("}\n}\n");
                }
                S::Loop(b) => {
                    out.push_str("'l: loop {\n");
                    stmts(b, muts, out);
                    out.push_str("}\n");
                }
                S::Continue => out.push_str("continue 'l;\n"),
                S::Ret(e) => {
                    let _ = writeln!(out, "return {};", ex(e));
                }
                S::Try(pt, e, r, h) => {
                    let _ = writeln!(out, "let {} = match {} {{\nOk(v) => v,\nErr({r}) => {{", pat(pt, muts), ex(e));
                    stmts(h, muts, out);
                    out.push_str("}\n};\n");
                }
                S::Res(e, o, a, r, b) => {
                    let _ = writeln!(out, "match {} {{\nOk({o}) => {{", ex(e));
                    stmts(a, muts, out);
                    let _ = writeln!(out, "}}\nErr({r}) => {{");
                    stmts(b, muts, out);
                    out.push_str("}\n}\n");
                }
                S::Fn(d) => print_fn(d, true, out),
                S::Unreachable => out.push_str("unreachable!()\n"),
                S::Comment(t) => {
                    let _ = writeln!(out, "// {t}");
                }
            }
        }
    }

    /// Print a top-level function definition.
    pub fn func(d: &FnDef, out: &mut String) {
        print_fn(d, false, out)
    }

    /// `nested`: attributes share the `fn` line, so a nested function never
    /// starts a line with `fn` and text slicing by top-level definitions
    /// stays valid.
    fn print_fn(d: &FnDef, nested: bool, out: &mut String) {
        let mut muts = std::collections::HashSet::new();
        assigned(&d.body, &mut muts);
        match d.inline {
            Inline::Always => out.push_str("#[inline(always)] "),
            Inline::Never => out.push_str("#[inline(never)] "),
            Inline::Default => {}
        }
        if d.cold {
            out.push_str("#[cold] ");
        }
        let mut ps: Vec<String> = Vec::new();
        if d.ctx {
            ps.push("ctx: &mut Wctx".into());
        }
        for (x, t) in &d.params {
            ps.push(format!("{}: {}", pat(&Pat::One(x.clone()), &muts), ty(*t)));
        }
        let r = if d.ret == Ty::Unit { String::new() } else { format!(" -> {}", ty(d.ret)) };
        let sep = if nested { ' ' } else { '\n' };
        let _ = writeln!(out, "#[allow(clippy::too_many_arguments)]{sep}fn {}({}){r} {{", d.name, ps.join(", "));
        stmts(&d.body, &muts, out);
        out.push_str("}\n\n");
    }
}
