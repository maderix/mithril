//! The CUDA printer of the lowered IR: `program.cu` from a `LirProgram`.
//!
//! Same decisions as the CPU program (they were made by the emitters);
//! this only spells them in C++ over the device runtime (`cuda/engine.cu`),
//! which supplies the IR's helper vocabulary without a context object.
//! Suspension results are `R` / `RA<k>` structs, tuples are `T<k>` /
//! `P2` structs, nested capture functions are lambdas, slice arguments
//! are hoisted to local arrays.

use mithril_codegen::lir::{Bop, FnDef, Pat, Ty, E, S};
use mithril_codegen::{LirProgram, Rule};
use mithril_net::{count_uses, ClosureSpec, Entry, MatchMeta, NExpr, ARR_PAIR};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;

struct P<'a> {
    /// return type of every generated function, by name
    fnret: &'a HashMap<String, Ty>,
    /// types of the locals in scope
    locals: HashMap<String, Ty>,
    /// names declared per open block (a redeclaration prints as assignment)
    scopes: Vec<HashSet<String>>,
    /// statements hoisted before the one being printed (slice arrays)
    pre: String,
    tmp: u32,
    /// the enclosing function's return type
    ret: Ty,
    out: String,
}

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
        Ty::ResArr(k) => format!("RA<{k}>"),
        Ty::Arr(k) => format!("A<{k}>"),
        Ty::Tup(k) => format!("T{k}"),
        Ty::RefU64 => "u64*".into(),
        Ty::RefU32 => "u32*".into(),
        Ty::RefI64 => "i64*".into(),
        Ty::Infer => "auto".into(),
        Ty::Unit => "void".into(),
    }
}

/// Return type of a helper of the device runtime (see engine.cu).
fn helper_ret(f: &str) -> Ty {
    match f {
        "as_i" | "sat_mul" | "imax" | "fuel_of" | "wrap56" | "floor_div" | "py_mod" | "sh" | "atomic_load" => Ty::I64,
        "con_tag" => Ty::U16,
        "con_ar" => Ty::U8,
        "con_addr" | "alloc2" | "alloc_rec" | "rec_d" | "rec_s" => Ty::U32,
        "arr_len_of" => Ty::Usize,
        "flag_load" | "is_err" => Ty::Bool,
        "apply" | "dive_res" | "dive_res_fork" => Ty::Res,
        "fork_fuel" => Ty::RefI64,
        _ if f.starts_with("f32_") => Ty::I64,
        _ => Ty::U64,
    }
}

impl<'a> P<'a> {
    fn fresh(&mut self, p: &str) -> String {
        self.tmp += 1;
        format!("{p}{}", self.tmp)
    }

    fn call_ret(&self, f: &str) -> Ty {
        if let Some(t) = self.fnret.get(f) {
            return *t;
        }
        if let Some(rest) = f.strip_prefix("untup::<").or_else(|| f.strip_prefix("consume_chain::<")) {
            let k: usize = rest.trim_end_matches('>').parse().unwrap_or(0);
            return Ty::Arr(k);
        }
        helper_ret(f)
    }

    /// The type an expression has (operand widths decide C's arithmetic).
    fn ty_of(&self, e: &E) -> Ty {
        match e {
            E::Int(_, t) => *t,
            E::Bool(_) => Ty::Bool,
            E::Flo(_) => Ty::I64,
            E::V(x) => self.locals.get(x).copied().unwrap_or(Ty::I64),
            E::Deref(x) => match self.locals.get(x) {
                Some(Ty::RefU64) => Ty::U64,
                Some(Ty::RefU32) => Ty::U32,
                _ => Ty::I64,
            },
            E::Ref(_) | E::Addr(_) => Ty::U64,
            E::Call { f, .. } => self.call_ret(f),
            E::Bin(op, a, _) => {
                if matches!(op, Bop::Lt | Bop::Le | Bop::Gt | Bop::Ge | Bop::Eq | Bop::Ne) {
                    Ty::Bool
                } else {
                    self.ty_of(a)
                }
            }
            E::Not(_) => Ty::Bool,
            E::Neg(_) => Ty::I64,
            E::Cast(_, t) => *t,
            E::Idx(..) => Ty::U64,
            E::Tup(xs) => Ty::Tup(xs.len()),
            E::Arr(xs) => Ty::Arr(xs.len()),
            E::Slice(_) => Ty::U64,
            E::Ok(_) | E::Err(_) => self.ret,
            E::Const(c) => match c.as_str() {
                "NOHOLE" => Ty::U32,
                c if c.starts_with("FOLD_EST") => Ty::I64,
                _ => Ty::U64,
            },
        }
    }

    fn ex(&mut self, e: &E) -> String {
        match e {
            E::Int(n, t) => match t {
                Ty::I64 => format!("({n}ll)"),
                Ty::U64 | Ty::Usize => format!("{}ull", *n as u64),
                Ty::U32 => format!("{n}u"),
                Ty::U16 => format!("((u16){n})"),
                Ty::U8 => format!("((u8){n})"),
                _ => format!("{n}"),
            },
            E::Bool(b) => b.to_string(),
            E::Flo(x) => format!("__longlong_as_double(0x{:016x}ll)", x.to_bits()),
            E::V(x) | E::Const(x) => x.clone(),
            E::Deref(x) => format!("(*{x})"),
            E::Ref(x) | E::Addr(x) => format!("(&{x})"),
            E::Call { f, args, .. } => {
                let name = f.replace("::<", "<");
                let mut a: Vec<String> = Vec::new();
                for x in args {
                    if let E::Addr(name) = x {
                        if let Some(Ty::Arr(k)) = self.locals.get(name) {
                            a.push(format!("{name}.a, {k}"));
                            continue;
                        }
                    }
                    if let E::Slice(xs) = x {
                        let n = xs.len();
                        let s = self.fresh("_s");
                        let items: Vec<String> = xs.iter().map(|y| self.ex(y)).collect();
                        let _ = writeln!(self.pre, "u64 {s}[{}] = {{{}}};", n.max(1), items.join(", "));
                        a.push(format!("{s}, {n}"));
                    } else {
                        a.push(self.ex(x));
                    }
                }
                format!("{name}({})", a.join(", "))
            }
            E::Bin(op, a, b) => {
                let t = self.ty_of(a);
                let (sa, sb) = (self.ex(a), self.ex(b));
                let ct = ty(t);
                let cmp = |o: &str| format!("(({ct})({sa}) {o} ({ct})({sb}))");
                match op {
                    Bop::Lt => cmp("<"),
                    Bop::Le => cmp("<="),
                    Bop::Gt => cmp(">"),
                    Bop::Ge => cmp(">="),
                    Bop::Eq => cmp("=="),
                    Bop::Ne => cmp("!="),
                    Bop::And => format!("(({sa}) & ({sb}))"),
                    Bop::Or => format!("(({sa}) | ({sb}))"),
                    Bop::Xor => format!("(({sa}) ^ ({sb}))"),
                    Bop::Add | Bop::Sub | Bop::Mul | Bop::Shl | Bop::Div | Bop::Shr => {
                        let o = match op {
                            Bop::Add => "+",
                            Bop::Sub => "-",
                            Bop::Mul => "*",
                            Bop::Shl => "<<",
                            Bop::Div => "/",
                            _ => ">>",
                        };
                        match (t, op) {
                            (Ty::I64, Bop::Div) => format!("idiv({sa}, {sb})"),
                            (Ty::I64, Bop::Shr) => format!("(({sa}) >> (({sb}) & 63))"),
                            (Ty::I64, Bop::Shl) => format!("((i64)((u64)({sa}) << (({sb}) & 63)))"),
                            // wrapping: through the unsigned width
                            (Ty::I64, _) => format!("((i64)((u64)({sa}) {o} (u64)({sb})))"),
                            (Ty::U32, Bop::Shl | Bop::Shr) => format!("((u32)({sa}) {o} (({sb}) & 31))"),
                            (Ty::U32, _) => format!("((u32)({sa}) {o} (u32)({sb}))"),
                            (_, Bop::Shl | Bop::Shr) => format!("(({sa}) {o} (({sb}) & 63))"),
                            _ => format!("(({sa}) {o} ({sb}))"),
                        }
                    }
                }
            }
            E::Not(a) => format!("(!({}))", self.ex(a)),
            E::Neg(a) => format!("(-({}))", self.ex(a)),
            E::Cast(a, t) => format!("(({})({}))", ty(*t), self.ex(a)),
            E::Idx(a, i) => format!("{}.a[{i}]", self.ex(a)),
            E::Tup(xs) => {
                let items: Vec<String> = xs.iter().map(|x| self.ex(x)).collect();
                format!("T{}{{{}}}", xs.len(), items.join(", "))
            }
            E::Arr(xs) => {
                let items: Vec<String> = xs.iter().map(|x| self.ex(x)).collect();
                format!("A<{}>{{{{{}}}}}", xs.len(), items.join(", "))
            }
            E::Slice(_) => unreachable!("slice outside an argument list"),
            E::Ok(a) => {
                let s = self.ex(a);
                match self.ret {
                    Ty::ResArr(k) => format!("RA<{k}>{{{s}, 0ull, true}}"),
                    _ => format!("R{{{s}, true}}"),
                }
            }
            E::Err(a) => {
                let s = self.ex(a);
                match self.ret {
                    Ty::ResArr(k) => format!("RA<{k}>{{A<{k}>{{}}, (u64)({s}), false}}"),
                    _ => format!("R{{(u64)({s}), false}}"),
                }
            }
        }
    }

    /// Flush hoisted declarations, then the statement text.
    fn line(&mut self, s: String) {
        let pre = std::mem::take(&mut self.pre);
        self.out.push_str(&pre);
        self.out.push_str(&s);
        self.out.push('\n');
    }

    /// Declare `x: t = init` (assignment when already declared in scope).
    fn decl(&mut self, x: &str, t: Ty, init: Option<String>) {
        let known = self.scopes.last().map_or(false, |s| s.contains(x));
        if known {
            if let Some(i) = init {
                self.line(format!("{x} = {i};"));
            }
            return;
        }
        if let Some(s) = self.scopes.last_mut() {
            s.insert(x.to_string());
        }
        self.locals.insert(x.to_string(), t);
        match init {
            Some(i) => self.line(format!("{} {x} = {i};", ty(t))),
            None => self.line(format!("{} {x};", ty(t))),
        }
    }

    fn block(&mut self, body: &[S]) {
        self.block_with(body, HashSet::new())
    }

    /// A block whose scope starts with `declared` (a function's parameters).
    fn block_with(&mut self, body: &[S], declared: HashSet<String>) {
        self.out.push_str("{\n");
        self.scopes.push(declared);
        for s in body {
            self.stmt(s);
        }
        self.scopes.pop();
        self.out.push_str("}\n");
    }

    /// The zero of the enclosing function's return type (after an abort).
    fn zero_ret(&self) -> String {
        match self.ret {
            Ty::Unit => "return;".into(),
            Ty::Res => "return R{0ull, true};".into(),
            Ty::ResArr(k) => format!("return RA<{k}>{{}};"),
            Ty::Tup(k) => format!("return T{k}{{}};"),
            t => format!("return ({})0;", ty(t)),
        }
    }

    fn stmt(&mut self, s: &S) {
        match s {
            S::Let(pat, t, e) => {
                let init = self.ex(e);
                match pat {
                    Pat::One(x) => {
                        let t = if *t == Ty::Infer { self.ty_of(e) } else { *t };
                        let t = if t == Ty::Infer { Ty::U64 } else { t };
                        self.decl(x, t, Some(init));
                    }
                    Pat::Tup(names) => {
                        let d = self.fresh("_d");
                        self.line(format!("auto {d} = {init};"));
                        let et = self.ty_of(e);
                        for (i, x) in names.iter().enumerate() {
                            let ft = match et {
                                Ty::Tup(_) => Ty::I64,
                                _ if i == 2 => Ty::U32, // consume2r's token
                                _ => Ty::U64,
                            };
                            self.decl(x, ft, Some(format!("{d}.f{i}")));
                        }
                    }
                    Pat::Arr(names) if names.is_empty() => {} // a nullary ctor has no cell
                    Pat::Arr(names) => {
                        let d = self.fresh("_d");
                        self.line(format!("auto {d} = {init};"));
                        for (i, x) in names.iter().enumerate() {
                            self.decl(x, Ty::U64, Some(format!("{d}.a[{i}]")));
                        }
                    }
                }
            }
            S::Decl(x, t) => self.decl(x, *t, None),
            S::Set(x, e) => {
                let v = self.ex(e);
                self.line(format!("{x} = {v};"));
            }
            S::Store(x, e) => {
                let v = self.ex(e);
                self.line(format!("(*{x}) = {v};"));
            }
            // the stack guard leaves the frame (value-initialized return)
            S::Do(E::Call { f, .. }) if f == "stack_guard" => self.line(format!("if (stack_deep()) return{};", if self.ret == Ty::Unit { "" } else { " {}" })),
            S::Do(e) => {
                let v = self.ex(e);
                self.line(format!("{v};"));
            }
            S::If(c, a, b) => {
                let cnd = self.ex(c);
                self.line(format!("if ({cnd}) "));
                self.block(a);
                if !b.is_empty() {
                    self.out.push_str("else ");
                    self.block(b);
                }
            }
            S::Switch(e, arms, d) => {
                let v = self.ex(e);
                self.line(format!("switch ({v}) {{"));
                for (k, body) in arms {
                    self.out.push_str(&format!("case {k}: "));
                    self.block(body);
                    self.out.push_str("break;\n");
                }
                self.out.push_str("default: ");
                match d {
                    Some(body) => self.block(body),
                    None => {
                        let z = self.zero_ret();
                        self.out.push_str(&format!("{{ mith_unreachable(); {z} }}\n"));
                    }
                }
                self.out.push_str("break;\n}\n");
            }
            S::Loop(body) => {
                self.out.push_str("for (;;) ");
                self.block(body);
            }
            S::Continue => self.out.push_str("continue;\n"),
            S::Ret(e) => {
                if matches!(e, E::Tup(xs) if xs.is_empty()) {
                    self.out.push_str("return;\n");
                } else {
                    let v = self.ex(e);
                    self.line(format!("return {v};"));
                }
            }
            S::Try(pat, e, r, h) => {
                let rt = self.ty_of(e);
                let v = self.ex(e);
                let rn = self.fresh("_r");
                // On the device the budget a dive-form call hands down is
                // refunded when the callee returns, so the budget bounds the
                // native recursion depth (the device stack is small) and not
                // the work of a subtree: a wide, shallow subtree runs without
                // suspending. (The CPU keeps the work budget: its suspension
                // is what exposes work to idle threads.)
                let hands_budget = matches!(e, E::Call { args, .. } if matches!(args.first(), Some(E::V(f)) if f == "fuel") || matches!(args.first(), Some(E::Call { f, .. }) if f == "fork_fuel"));
                if hands_budget {
                    self.line(format!("i64 {rn}_keep = *fuel; auto {rn} = {v}; *fuel = {rn}_keep;"));
                } else {
                    self.line(format!("auto {rn} = {v};"));
                }
                self.out.push_str(&format!("if (!{rn}.ok) "));
                let recf = if matches!(rt, Ty::ResArr(_)) { "rec" } else { "v" };
                let mut hb = vec![];
                if r != "_" {
                    hb.push(S::Let(Pat::One(r.clone()), Ty::U32, E::Cast(Box::new(E::V(format!("{rn}.{recf}"))), Ty::U32)));
                }
                hb.extend(h.iter().cloned());
                self.block(&hb);
                let Pat::One(x) = pat else { unreachable!("try binds one value") };
                match rt {
                    Ty::ResArr(k) => self.decl(x, Ty::Arr(k), Some(format!("{rn}.a"))),
                    _ => self.decl(x, Ty::U64, Some(format!("{rn}.v"))),
                }
            }
            S::Res(e, o, a, r, b) => {
                let rt = self.ty_of(e);
                let v = self.ex(e);
                let rn = self.fresh("_r");
                self.line(format!("auto {rn} = {v};"));
                self.out.push_str(&format!("if ({rn}.ok) "));
                let mut ab = vec![match rt {
                    Ty::ResArr(k) => S::Let(Pat::One(o.clone()), Ty::Arr(k), E::V(format!("{rn}.a"))),
                    _ => S::Let(Pat::One(o.clone()), Ty::U64, E::V(format!("{rn}.v"))),
                }];
                ab.extend(a.iter().cloned());
                self.block(&ab);
                self.out.push_str("else ");
                let recf = if matches!(rt, Ty::ResArr(_)) { "rec" } else { "v" };
                let mut bb = vec![S::Let(Pat::One(r.clone()), Ty::U32, E::Cast(Box::new(E::V(format!("{rn}.{recf}"))), Ty::U32))];
                bb.extend(b.iter().cloned());
                self.block(&bb);
            }
            S::Fn(d) => {
                // a lambda over explicit parameters (no captures)
                let params: Vec<String> = d.params.iter().map(|(x, t)| format!("{} {x}", ty(*t))).collect();
                self.out.push_str(&format!("auto {} = [](
{}) -> {} ", d.name, params.join(", "), ty(d.ret)));
                let saved = (std::mem::take(&mut self.locals), self.ret);
                self.locals = d.params.iter().cloned().collect();
                self.ret = d.ret;
                self.block_with(&d.body, d.params.iter().map(|(x, _)| x.clone()).collect());
                self.out.push_str(";\n");
                self.locals = saved.0;
                self.ret = saved.1;
                self.locals.insert(d.name.clone(), d.ret);
            }
            S::Unreachable => {
                let z = self.zero_ret();
                self.out.push_str(&format!("mith_unreachable(); {z}\n"));
            }
            S::Comment(t) => {
                self.out.push_str(&format!("// {t}\n"));
            }
        }
    }
}

/// Every `T<k>` width a statement list mentions.
fn tuple_widths(body: &[S], out: &mut Vec<usize>) {
    fn ex(e: &E, out: &mut Vec<usize>) {
        match e {
            E::Tup(xs) => {
                if !xs.is_empty() {
                    out.push(xs.len());
                }
                xs.iter().for_each(|x| ex(x, out));
            }
            E::Call { args, .. } | E::Arr(args) | E::Slice(args) => args.iter().for_each(|a| ex(a, out)),
            E::Bin(_, a, b) => {
                ex(a, out);
                ex(b, out);
            }
            E::Not(a) | E::Neg(a) | E::Cast(a, _) | E::Idx(a, _) | E::Ok(a) | E::Err(a) => ex(a, out),
            _ => {}
        }
    }
    for s in body {
        match s {
            S::Let(_, t, e) => {
                if let Ty::Tup(k) = t {
                    out.push(*k);
                }
                ex(e, out);
            }
            S::Decl(_, Ty::Tup(k)) => out.push(*k),
            S::Set(_, e) | S::Store(_, e) | S::Do(e) | S::Ret(e) => ex(e, out),
            S::If(e, a, b) => {
                ex(e, out);
                tuple_widths(a, out);
                tuple_widths(b, out);
            }
            S::Switch(e, arms, d) => {
                ex(e, out);
                arms.iter().for_each(|(_, b)| tuple_widths(b, out));
                d.iter().for_each(|b| tuple_widths(b, out));
            }
            S::Loop(b) => tuple_widths(b, out),
            S::Try(_, e, _, h) => {
                ex(e, out);
                tuple_widths(h, out);
            }
            S::Res(e, _, a, _, b) => {
                ex(e, out);
                tuple_widths(a, out);
                tuple_widths(b, out);
            }
            S::Fn(d) => {
                if let Ty::Tup(k) = d.ret {
                    out.push(k);
                }
                tuple_widths(&d.body, out);
            }
            _ => {}
        }
    }
}

fn signature(d: &FnDef) -> String {
    let params: Vec<String> = d.params.iter().map(|(x, t)| format!("{} {x}", ty(*t))).collect();
    format!("__device__ {} {}({})", ty(d.ret), d.name, params.join(", "))
}

/// Print `program.cu` for a lowered program.
pub fn print(prog: &LirProgram) -> String {
    let fnret: HashMap<String, Ty> = prog.fns.iter().map(|d| (d.name.clone(), d.ret)).collect();
    let mut out = String::new();
    let _ = writeln!(out, "// program.cu generated by mithril-gpu; do not edit.");
    let _ = writeln!(out, "#define PROG_NRULES {}", prog.rules.len());
    let _ = writeln!(out, "#define NFNS {}", prog.dives.len());
    let _ = writeln!(out, "#define NET_RULE {}", prog.net_rule);
    let _ = writeln!(out, "#define FILL_RULE {}", prog.fill_rule);
    let _ = writeln!(out, "#define FWD_RULE {}", prog.fwd);
    let _ = writeln!(out, "#define FIELD_RULE {}\n#define WHOLE_RULE {}\n#define RELINK_RULE {}", prog.settle_rules[0], prog.settle_rules[1], prog.settle_rules[2]);
    let _ = writeln!(out, "#include \"engine.cu\"\n");
    // tables
    let lin: Vec<&str> = prog.lin.iter().map(|b| if *b { "true" } else { "false" }).collect();
    let _ = writeln!(out, "__device__ const bool LIN[{}] = {{{}}};", prog.lin.len().max(1), if lin.is_empty() { "false".to_string() } else { lin.join(", ") });
    let _ = writeln!(out, "__device__ bool lin(u16 k) {{ return k == 0xfffu ? {} : LIN[k]; }}", prog.lin_tup);
    let cids: Vec<String> = prog.unbox_cid.iter().map(|c| c.to_string()).collect();
    let _ = writeln!(out, "__device__ const u32 UNBOX_CID[{}] = {{{}}};", prog.unbox_cid.len().max(1), if cids.is_empty() { "0".to_string() } else { cids.join(", ") });
    let _ = writeln!(out, "__device__ u32 unbox_cid(u64 slot) {{ return UNBOX_CID[slot]; }}");
    let rr: Vec<&str> = prog.rules.iter().map(|r| if matches!(r, Rule::Seg(_) | Rule::Join(_) | Rule::Hole(_) | Rule::Fill | Rule::Field | Rule::Whole | Rule::Relink) { "true" } else { "false" }).collect();
    let _ = writeln!(out, "__device__ const bool REC_RULE[PROG_NRULES] = {{{}}};\n__device__ bool prog_rec_rule(u32 rule) {{ return REC_RULE[rule]; }}", rr.join(", "));
    // rules that can fork (a call rule, a segment with a call, the net
    // region's rules): a GROW sweep fires only these
    let forks: Vec<&str> = (0..prog.rules.len()).map(|r| if prog.diving.contains(&(r as u16)) { "1" } else { "0" }).collect();
    let _ = writeln!(out, "extern \"C\" __device__ const unsigned char FORKS[PROG_NRULES] = {{{}}};\n__device__ bool prog_forks(u32 rule) {{ return FORKS[rule] != 0; }}", forks.join(", "));
    for f in &prog.folds {
        let _ = writeln!(out, "__device__ i64 FOLD_EST_{f} = 1;\n__device__ int FOLD_MEAS_{f} = 0;");
    }
    // native int tuples of every width the program uses
    let mut widths: Vec<usize> = Vec::new();
    for d in &prog.fns {
        if let Ty::Tup(k) = d.ret {
            widths.push(k);
        }
        tuple_widths(&d.body, &mut widths);
    }
    widths.sort_unstable();
    widths.dedup();
    for k in widths {
        let fs: Vec<String> = (0..k).map(|i| format!("f{i}")).collect();
        let _ = writeln!(out, "struct T{k} {{ i64 {}; }};", if fs.is_empty() { "_z".to_string() } else { fs.join(", ") });
    }
    out.push('\n');
    for d in &prog.fns {
        let _ = writeln!(out, "{};", signature(d));
    }
    out.push('\n');
    for d in &prog.fns {
        let mut p = P { fnret: &fnret, locals: d.params.iter().cloned().collect(), scopes: Vec::new(), pre: String::new(), tmp: 0, ret: d.ret, out: String::new() };
        p.out.push_str(&signature(d));
        p.out.push(' ');
        p.block_with(&d.body, d.params.iter().map(|(x, _)| x.clone()).collect());
        // C++ wants a return on every path; the IR ends with one or diverges
        out.push_str(&p.out);
        out.push('\n');
    }
    // prog_dive: args[0] is the destination, the rest the arguments
    let _ = writeln!(out, "__device__ R prog_dive(u32 f, const u64 *args, i64 *fuel) {{\n  switch (f) {{");
    for (fid, (ar, bor)) in prog.dives.iter().enumerate() {
        let args: String = (0..*ar).map(|i| format!(", args[{}]", i + 1)).collect();
        let lent: String = (0..*ar).filter(|&i| bor[i]).map(|i| format!(" free_val(args[{}]);", i + 1)).collect();
        let _ = writeln!(out, "  case {fid}u: {{ R r = d_{fid}(fuel{args});{lent} return r; }}");
    }
    let _ = writeln!(out, "  default: g_abort(AB_UNREACHABLE); return R{{0ull, true}};\n  }}\n}}\n");
    // prog_fire: the rule table
    let _ = writeln!(out, "__device__ void prog_fire(u32 rule, u64 e0, u64 e1, u64 e2) {{\n  switch (rule) {{");
    for (id, r) in prog.rules.iter().enumerate() {
        let call = match r {
            Rule::Boot(f) => format!("fc_{f}(e0, e1, ROOT)"),
            Rule::Call(f) => format!("fc_{f}(e0, e1, e2)"),
            Rule::Join(f) => format!("jn_{f}(e0, e1, e2)"),
            Rule::Seg(k) => format!("sg_{k}(e0, e1, e2)"),
            Rule::Hole(k) => format!("hl_{k}(e0, e1, e2)"),
            Rule::Net => "net_fire(e0, e1)".to_string(),
            Rule::Fill => "fill_fire(e0, e2)".to_string(),
            Rule::Field => "field_fire(e0, e2)".to_string(),
            Rule::Whole => "whole_fire(e2)".to_string(),
            Rule::Relink => "relink_fire(e2)".to_string(),
        };
        let _ = writeln!(out, "  case {id}u: {call}; return;");
    }
    let _ = writeln!(out, "  default: g_abort(AB_UNREACHABLE); return;\n  }}\n}}");
    net_region(prog, &mut out);
    out
}

// ---- the net region: entries as net builders, the match tables ----

/// The instantiation of an entry (`mithril_core::lower::instantiate`)
/// spelled as straight-line C: every wire and cell a local, every
/// variable's uses fanned out by the same DUP chains.
struct Inst {
    out: String,
    tmp: u32,
    /// var -> the ports its remaining uses read, front first
    env: HashMap<u32, VecDeque<String>>,
}

impl Inst {
    fn fresh(&mut self) -> String {
        self.tmp += 1;
        format!("w{}", self.tmp)
    }
    fn stmt(&mut self, s: String) {
        self.out.push_str("  ");
        self.out.push_str(&s);
        self.out.push('\n');
    }
    fn take(&mut self, v: u32) -> String {
        self.env.get_mut(&v).and_then(|q| q.pop_front()).unwrap_or_else(|| panic!("ICE: use of var v{v} exceeds its counted uses"))
    }
    /// Bind `v` to `port` for `k` uses: 0 erases, 1 passes through, k > 1
    /// fans out through a chain of k-1 dups (one fresh label each).
    fn bind(&mut self, v: u32, port: String, k: usize) {
        match k {
            0 => self.stmt(format!("link(era(), {port});")),
            1 => self.env.entry(v).or_default().push_back(port),
            _ => {
                let outs: Vec<String> = (0..k).map(|_| self.fresh()).collect();
                for o in &outs {
                    self.stmt(format!("u64 {o} = wire();"));
                }
                let mut next = outs[k - 1].clone();
                for j in (0..k - 1).rev() {
                    let d = self.fresh();
                    self.stmt(format!("u64 {d} = dup_port(alloc_node({}, {next}), fresh_label());", outs[j]));
                    next = d;
                }
                self.stmt(format!("link({next}, {port});"));
                self.env.entry(v).or_default().extend(outs);
            }
        }
    }
    fn closure(&mut self, spec: &ClosureSpec) -> String {
        let caps: Vec<String> = spec.caps.iter().map(|v| self.take(*v)).collect();
        let l = self.list(&caps);
        format!("ref_port({l}, {})", spec.entry)
    }
    /// A list chain of `items` (a local array + `list_alloc`).
    fn list(&mut self, items: &[String]) -> String {
        let a = self.fresh();
        self.stmt(format!("u64 {a}[{}] = {{{}}};", items.len().max(1), if items.is_empty() { "0".to_string() } else { items.join(", ") }));
        let h = self.fresh();
        self.stmt(format!("u64 {h} = list_alloc({a}, {});", items.len()));
        h
    }
    /// `[p2, w]` cell fed by `p1` through an op: the result wire.
    fn binop(&mut self, code: u16, p1: String, p2: String) -> String {
        let w = self.fresh();
        self.stmt(format!("u64 {w} = wire();"));
        self.stmt(format!("link(op_port(alloc_node({p2}, {w}), {code}), {p1});"));
        w
    }
    fn inst(&mut self, e: &NExpr) -> String {
        match e {
            NExpr::Num(n) => format!("num({n}ll)"),
            NExpr::Flo(f) => format!("alloc_flo(__longlong_as_double(0x{:016x}ll))", f.to_bits()),
            NExpr::Var(v) => self.take(*v),
            NExpr::Op2(code, a, b) => {
                let p1 = self.inst(a);
                let p2 = self.inst(b);
                self.binop(*code, p1, p2)
            }
            NExpr::Let(v, r, b) => {
                let pr = self.inst(r);
                self.bind(*v, pr, count_uses(*v, b));
                self.inst(b)
            }
            NExpr::Call(f, args) => {
                let ps: Vec<String> = args.iter().map(|a| self.inst(a)).collect();
                let h = self.list(&ps);
                let w = self.fresh();
                self.stmt(format!("u64 {w} = wire();"));
                self.stmt(format!("push_redex(ref_port({h}, {f}), {w});"));
                w
            }
            NExpr::Ctor(_, args) | NExpr::Tuple(args) => {
                let ctag = if let NExpr::Ctor(t, _) = e { *t } else { 0xFFF };
                let ps: Vec<String> = args.iter().map(|a| self.inst(a)).collect();
                let a = self.fresh();
                self.stmt(format!("u64 {a}[{}] = {{{}}};", ps.len().max(1), if ps.is_empty() { "0".to_string() } else { ps.join(", ") }));
                let c = self.fresh();
                self.stmt(format!("u64 {c} = con_alloc({ctag}, {a}, {});", ps.len()));
                c
            }
            NExpr::If(c, t, e2) => {
                let pc = self.inst(c);
                let rt = self.closure(t);
                let re = self.closure(e2);
                let w = self.fresh();
                self.stmt(format!("u64 {w} = wire();"));
                self.stmt(format!("link(mkport(T_SWI, alloc_node({w}, mkport(T_EXT, alloc_node({rt}, {re})))), {pc});"));
                w
            }
            NExpr::Match(s, mid, specs) => {
                let ps = self.inst(s);
                let refs: Vec<String> = specs.iter().map(|sp| self.closure(sp)).collect();
                let h = self.list(&refs);
                let w = self.fresh();
                self.stmt(format!("u64 {w} = wire();"));
                self.stmt(format!("link(mat_port(alloc_node({w}, {h}), {mid}), {ps});"));
                w
            }
            NExpr::Proj(e2, mid) => {
                let pe = self.inst(e2);
                let w = self.fresh();
                self.stmt(format!("u64 {w} = wire();"));
                self.stmt(format!("link(mat_port(alloc_node({w}, EMPTY), {mid}), {pe});"));
                w
            }
            NExpr::Prim(code, args) => {
                let ps: Vec<String> = args.iter().map(|a| self.inst(a)).collect();
                match ps.len() {
                    1 => self.binop(*code, ps[0].clone(), "num(0ll)".into()),
                    2 => self.binop(*code, ps[0].clone(), ps[1].clone()),
                    3 => {
                        let pair = self.binop(ARR_PAIR, ps[1].clone(), ps[2].clone());
                        self.binop(*code, ps[0].clone(), pair)
                    }
                    n => panic!("ICE: builtin with {n} arguments"),
                }
            }
            NExpr::Lam(x, body) => {
                let p = self.fresh();
                let b = self.fresh();
                self.stmt(format!("u64 {p} = wire();"));
                self.stmt(format!("u64 {b} = wire();"));
                self.bind(*x, p.clone(), count_uses(*x, body));
                let o = self.inst(body);
                self.stmt(format!("link({o}, {b});"));
                format!("mkport(T_LAM, alloc_node({p}, {b}))")
            }
            NExpr::App(f, a) => {
                let pf = self.inst(f);
                let pa = self.inst(a);
                let w = self.fresh();
                self.stmt(format!("u64 {w} = wire();"));
                self.stmt(format!("link(mkport(T_APP, alloc_node({pa}, {w})), {pf});"));
                w
            }
        }
    }
}

fn entry_builder(id: usize, e: &Entry) -> String {
    let mut g = Inst { out: String::new(), tmp: 0, env: HashMap::new() };
    for (i, p) in e.params.iter().enumerate() {
        let k = count_uses(*p, &e.body);
        g.bind(*p, format!("args[{i}]"), k);
    }
    let o = g.inst(&e.body);
    g.stmt(format!("link({o}, ret);"));
    format!("// entry {id}: params {:?}\n__device__ void inst_{id}(const u64 *args, int n, u64 ret) {{\n  if (n != {}) {{ g_abort(AB_UNREACHABLE); return; }}\n{}}}\n", e.params, e.params.len(), g.out)
}

fn net_region(prog: &LirProgram, out: &mut String) {
    let np = &prog.net;
    for (i, e) in np.entries.iter().enumerate() {
        if prog.net_live[i] {
            out.push_str(&entry_builder(i, e));
        }
    }
    let _ = writeln!(out, "__device__ void prog_inst(u16 entry, const u64 *args, int n, u64 ret) {{\n  switch (entry) {{");
    for (i, _) in np.entries.iter().enumerate() {
        if prog.net_live[i] {
            let _ = writeln!(out, "  case {i}: inst_{i}(args, n, ret); return;");
        }
    }
    let _ = writeln!(out, "  default: g_abort(AB_UNREACHABLE); return;\n  }}\n}}");
    // match tables
    let _ = writeln!(out, "__device__ bool prog_mat_proj(u16 mid, usize *i) {{\n  switch (mid) {{");
    for (mid, mm) in np.metas.iter().enumerate() {
        if let MatchMeta::Proj(k) = mm {
            let _ = writeln!(out, "  case {mid}: *i = {k}; return true;");
        }
    }
    let _ = writeln!(out, "  default: return false;\n  }}\n}}");
    for (mid, mm) in np.metas.iter().enumerate() {
        if let MatchMeta::Arms(tags) = mm {
            let ts: Vec<String> = tags.iter().map(|t| t.to_string()).collect();
            let _ = writeln!(out, "__device__ const u16 MAT_{mid}[{}] = {{{}}};", tags.len().max(1), if ts.is_empty() { "0".to_string() } else { ts.join(", ") });
        }
    }
    let _ = writeln!(out, "__device__ const u16 *prog_mat_arms(u16 mid, int *n) {{\n  switch (mid) {{");
    for (mid, mm) in np.metas.iter().enumerate() {
        if let MatchMeta::Arms(tags) = mm {
            let _ = writeln!(out, "  case {mid}: *n = {}; return MAT_{mid};", tags.len());
        }
    }
    let _ = writeln!(out, "  default: *n = 0; return 0;\n  }}\n}}");
}
