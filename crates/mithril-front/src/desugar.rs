//! Desugars the surface `Module` (statements, mutation, loops) into the
//! purely-functional `Core` IR: `while`/`for` become fresh self-recursive
//! top-level functions; `if`/`match` become `Core::If`/`Core::Match` with
//! their non-returning arms merged via a live-variable tuple (SSA-style).
//!
//! Desugar-time diagnostics can't carry a real source line (the surface
//! `Expr`/`Stmt` types have no line field), so every `Diag` here uses
//! line `0`; this is a known, documented limitation of the given AST.

use crate::ast::{BinOp, BoolOp, CmpOp, Expr, FnDef, Module, Pat, Stmt};
use crate::ast::{Combiner as AstCombiner, FoldInfo as AstFoldInfo};
use crate::core::{self, Combiner, Core, CoreFn, CoreModule, CtorId, FnId, UNREACHABLE_CTOR};
use crate::Diag;
use std::collections::{BTreeSet, HashMap, HashSet};

pub fn desugar(m: &Module) -> Result<CoreModule, Diag> {
    let mut fn_table = HashMap::new();
    let mut fn_arity = Vec::new();
    for (i, f) in m.fns.iter().enumerate() {
        if fn_table.insert(f.name.clone(), i as u32).is_some() {
            return Err(Diag::new(0, format!("duplicate function definition: {}", f.name)));
        }
        fn_arity.push(f.params.len());
    }
    let mut ctor_table = HashMap::new();
    let mut ctor_owner = HashMap::new();
    let mut data_ctors = HashMap::new();
    let mut ctors_flat: Vec<(String, usize)> = Vec::new();
    for d in &m.datas {
        let mut names = Vec::new();
        for (cname, binds) in &d.ctors {
            if ctor_table.contains_key(cname) {
                return Err(Diag::new(0, format!("duplicate constructor definition: {}", cname)));
            }
            let cid = ctors_flat.len() as u32;
            ctor_table.insert(cname.clone(), (cid, binds.len()));
            ctor_owner.insert(cname.clone(), d.name.clone());
            ctors_flat.push((cname.clone(), binds.len()));
            names.push(cname.clone());
        }
        data_ctors.insert(d.name.clone(), names);
    }
    let t = Tables { fn_table, fn_arity, ctor_table, ctor_owner, data_ctors };
    let mut g = Gen { out_fns: Vec::new() };
    for _ in &m.fns {
        g.out_fns.push(placeholder());
    }
    for (i, f) in m.fns.iter().enumerate() {
        let cf = compile_fn(f, &t, &mut g)?;
        g.out_fns[i] = cf;
    }
    // `main` is informational metadata for later tasks (codegen/CLI entry
    // point); default to the first function if none is named `main`.
    let main = t.fn_table.get("main").copied().unwrap_or(0);
    Ok(CoreModule { fns: g.out_fns, ctors: ctors_flat, main })
}

fn placeholder() -> CoreFn {
    CoreFn { name: String::new(), arity: 0, body: Core::Num(0), self_tail_rec: true, fold: None }
}

/// Immutable, precomputed name-resolution tables.
struct Tables {
    fn_table: HashMap<String, FnId>,
    fn_arity: Vec<usize>,
    ctor_table: HashMap<String, (CtorId, usize)>,
    ctor_owner: HashMap<String, String>,
    data_ctors: HashMap<String, Vec<String>>,
}

/// Growing list of top-level Core functions (user fns + generated loop
/// helpers), and the counter for fresh helper `FnId`s.
struct Gen {
    out_fns: Vec<CoreFn>,
}
impl Gen {
    fn fresh_fn_id(&mut self) -> FnId {
        let id = self.out_fns.len() as u32;
        self.out_fns.push(placeholder());
        id
    }
}

/// Per-function variable resolution: name -> current `Core::Var` index,
/// plus which names are currently known (by syntactic classification) to
/// hold a `bool` value, used to enforce "only bool in conditions".
#[derive(Clone)]
struct Scope {
    vars: HashMap<String, u32>,
    bool_vars: HashSet<String>,
    next_idx: u32,
}
impl Scope {
    fn new() -> Scope {
        Scope { vars: HashMap::new(), bool_vars: HashSet::new(), next_idx: 0 }
    }
    fn fresh(&mut self, name: &str) -> u32 {
        let i = self.next_idx;
        self.next_idx += 1;
        self.vars.insert(name.to_string(), i);
        i
    }
    fn fresh_anon(&mut self) -> u32 {
        let i = self.next_idx;
        self.next_idx += 1;
        i
    }
    fn get(&self, name: &str) -> Result<u32, Diag> {
        self.vars.get(name).copied().ok_or_else(|| Diag::new(0, format!("unknown variable: {}", name)))
    }
}

// ---- boolean-condition classification ----

fn is_boolish(e: &Expr, bool_vars: &HashSet<String>) -> bool {
    match e {
        Expr::Bool(_) | Expr::Cmp(..) | Expr::Bool2(..) | Expr::Not(_) => true,
        Expr::IfExp(_, t, el) => is_boolish(t, bool_vars) && is_boolish(el, bool_vars),
        Expr::Var(n) => bool_vars.contains(n),
        _ => false,
    }
}

fn check_cond(e: &Expr, scope: &Scope) -> Result<(), Diag> {
    if is_boolish(e, &scope.bool_vars) {
        Ok(())
    } else {
        Err(Diag::new(0, "condition must be bool; int (and other non-bool values) are not allowed in conditions"))
    }
}

// ---- static analysis over surface statements ----

/// True iff every control-flow path through `stmts` ends in a `return`.
fn always_returns(stmts: &[Stmt]) -> bool {
    match stmts.last() {
        Some(Stmt::Return(_)) => true,
        Some(Stmt::If(_, t, e)) => always_returns(t) && always_returns(e),
        Some(Stmt::Match(_, cases)) => !cases.is_empty() && cases.iter().all(|(_, b)| always_returns(b)),
        _ => false,
    }
}

fn contains_return(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Return(_) => true,
        Stmt::If(_, t, e) => contains_return(t) || contains_return(e),
        Stmt::While(_, b) => contains_return(b),
        Stmt::For(_, _, b, _) => contains_return(b),
        Stmt::Match(_, cases) => cases.iter().any(|(_, b)| contains_return(b)),
        _ => false,
    })
}

/// Names assigned anywhere within `stmts` (recursing into nested blocks).
/// A `for` loop's induction variable counts as assigned by the loop.
fn assigned_names(stmts: &[Stmt]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    fn go(stmts: &[Stmt], out: &mut BTreeSet<String>) {
        for s in stmts {
            match s {
                Stmt::Assign(n, _) => {
                    out.insert(n.clone());
                }
                Stmt::If(_, t, e) => {
                    go(t, out);
                    go(e, out);
                }
                Stmt::While(_, b) => go(b, out),
                Stmt::For(v, _, b, _) => {
                    out.insert(v.clone());
                    go(b, out);
                }
                Stmt::Match(_, cases) => {
                    for (_, b) in cases {
                        go(b, out);
                    }
                }
                Stmt::Return(_) | Stmt::ExprStmt(_) => {}
            }
        }
    }
    go(stmts, &mut out);
    out
}

fn free_reads_expr(e: &Expr, out: &mut BTreeSet<String>) {
    match e {
        Expr::Var(n) => {
            out.insert(n.clone());
        }
        Expr::Bin(_, a, b) | Expr::Cmp(_, a, b) | Expr::Bool2(_, a, b) | Expr::Index(a, b) => {
            free_reads_expr(a, out);
            free_reads_expr(b, out);
        }
        Expr::Not(a) => free_reads_expr(a, out),
        Expr::IfExp(c, t, e2) => {
            free_reads_expr(c, out);
            free_reads_expr(t, out);
            free_reads_expr(e2, out);
        }
        Expr::Call(_, args) | Expr::Tuple(args) => {
            for a in args {
                free_reads_expr(a, out);
            }
        }
        Expr::Lambda(params, body) => {
            let mut inner = BTreeSet::new();
            free_reads_expr(body, &mut inner);
            for p in params {
                inner.remove(p);
            }
            out.extend(inner);
        }
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) => {}
    }
}

fn free_reads_stmts(stmts: &[Stmt], out: &mut BTreeSet<String>) {
    for s in stmts {
        match s {
            Stmt::Assign(_, e) | Stmt::Return(e) | Stmt::ExprStmt(e) => free_reads_expr(e, out),
            Stmt::If(c, t, el) => {
                free_reads_expr(c, out);
                free_reads_stmts(t, out);
                free_reads_stmts(el, out);
            }
            Stmt::While(c, b) => {
                free_reads_expr(c, out);
                free_reads_stmts(b, out);
            }
            Stmt::For(_, bound, b, _) => {
                free_reads_expr(bound, out);
                free_reads_stmts(b, out);
            }
            Stmt::Match(scrut, cases) => {
                free_reads_expr(scrut, out);
                for (_, b) in cases {
                    free_reads_stmts(b, out);
                }
            }
        }
    }
}

// ---- expression compilation ----

fn compile_expr(e: &Expr, scope: &Scope, t: &Tables) -> Result<Core, Diag> {
    match e {
        Expr::Int(n) => Ok(Core::Num(*n)),
        Expr::Float(n) => Ok(Core::Flo(*n)),
        Expr::Bool(b) => Ok(Core::Num(if *b { 1 } else { 0 })),
        Expr::Var(n) => match scope.get(n) {
            Ok(v) => Ok(Core::Var(v)),
            // a top-level function used as a value: eta-expanded to a
            // (curried) closure over its arity
            Err(e) => match t.fn_table.get(n) {
                Some(&fid) => {
                    let arity = t.fn_arity[fid as usize];
                    let mut s = scope.clone();
                    let params: Vec<u32> = (0..arity).map(|_| s.fresh_anon()).collect();
                    let mut body = Core::Call(fid, params.iter().map(|p| Core::Var(*p)).collect());
                    for p in params.into_iter().rev() {
                        body = Core::Lam(p, Box::new(body));
                    }
                    Ok(body)
                }
                None => Err(e),
            },
        },
        Expr::Bin(op, a, b) => {
            Ok(Core::Op2(*op, Box::new(compile_expr(a, scope, t)?), Box::new(compile_expr(b, scope, t)?)))
        }
        Expr::Cmp(op, a, b) => {
            Ok(Core::Cmp(*op, Box::new(compile_expr(a, scope, t)?), Box::new(compile_expr(b, scope, t)?)))
        }
        Expr::Bool2(BoolOp::And, a, b) => {
            Ok(Core::If(Box::new(compile_expr(a, scope, t)?), Box::new(compile_expr(b, scope, t)?), Box::new(Core::Num(0))))
        }
        Expr::Bool2(BoolOp::Or, a, b) => {
            Ok(Core::If(Box::new(compile_expr(a, scope, t)?), Box::new(Core::Num(1)), Box::new(compile_expr(b, scope, t)?)))
        }
        Expr::Not(a) => Ok(Core::If(Box::new(compile_expr(a, scope, t)?), Box::new(Core::Num(0)), Box::new(Core::Num(1)))),
        Expr::IfExp(c, then, els) => {
            check_cond(c, scope)?;
            Ok(Core::If(
                Box::new(compile_expr(c, scope, t)?),
                Box::new(compile_expr(then, scope, t)?),
                Box::new(compile_expr(els, scope, t)?),
            ))
        }
        Expr::Call(name, args) => {
            let cargs: Vec<Core> = args.iter().map(|a| compile_expr(a, scope, t)).collect::<Result<_, _>>()?;
            if let Ok(f) = scope.get(name) {
                // a local variable applied: a closure call, one argument
                // at a time
                let mut e = Core::Var(f);
                for a in cargs {
                    e = Core::App(Box::new(e), Box::new(a));
                }
                Ok(e)
            } else if let Some(&(cid, arity)) = t.ctor_table.get(name) {
                if cargs.len() != arity {
                    return Err(Diag::new(0, format!("constructor '{}' expects {} arg(s), got {}", name, arity, cargs.len())));
                }
                Ok(Core::Ctor(cid, cargs))
            } else if let (None, Some((p, arity))) = (t.fn_table.get(name), crate::core::Prim::by_name(name)) {
                if cargs.len() != arity {
                    return Err(Diag::new(0, format!("builtin '{}' expects {} arg(s), got {}", name, arity, cargs.len())));
                }
                Ok(Core::Prim(p, cargs))
            } else if let Some(&fid) = t.fn_table.get(name) {
                let arity = t.fn_arity[fid as usize];
                if cargs.len() != arity {
                    return Err(Diag::new(0, format!("function '{}' expects {} arg(s), got {}", name, arity, cargs.len())));
                }
                Ok(Core::Call(fid, cargs))
            } else {
                Err(Diag::new(0, format!("unknown function or constructor: {}", name)))
            }
        }
        Expr::Tuple(items) => Ok(Core::Tuple(items.iter().map(|it| compile_expr(it, scope, t)).collect::<Result<_, _>>()?)),
        Expr::Index(base, idx) => match idx.as_ref() {
            Expr::Int(n) if *n >= 0 => Ok(Core::Proj(Box::new(compile_expr(base, scope, t)?), *n as usize)),
            _ => Err(Diag::new(0, "tuple index must be a non-negative integer literal")),
        },
        Expr::Lambda(params, body) => {
            // curried: each parameter is a fresh variable of an inner scope
            let mut s = scope.clone();
            let idxs: Vec<u32> = params.iter().map(|p| { s.bool_vars.remove(p); s.fresh(p) }).collect();
            let mut e = compile_expr(body, &s, t)?;
            for i in idxs.into_iter().rev() {
                e = Core::Lam(i, Box::new(e));
            }
            Ok(e)
        }
    }
}

// ---- statement-list compilation ----
//
// Every statement list is compiled with an explicit continuation `k`,
// invoked once the list is exhausted, producing the `Core` for whatever
// comes after this block (the rest of the enclosing function, ultimately).
// `k` takes `&mut Gen` as an explicit *argument* (rather than capturing it)
// so that closures built from it stay plain `Fn` values while still being
// able to mint fresh helper functions when invoked.
type Cont<'c> = dyn Fn(&Scope, &mut Gen) -> Result<Core, Diag> + 'c;

fn unreachable_tail(_: &Scope, _: &mut Gen) -> Result<Core, Diag> {
    Ok(Core::Ctor(UNREACHABLE_CTOR, vec![]))
}

fn compile_block(stmts: &[Stmt], scope: &mut Scope, t: &Tables, g: &mut Gen, k: &Cont) -> Result<Core, Diag> {
    let (head, rest) = match stmts.split_first() {
        None => return k(scope, g),
        Some(x) => x,
    };
    match head {
        Stmt::Assign(name, e) => {
            let ce = compile_expr(e, scope, t)?;
            let boolish = is_boolish(e, &scope.bool_vars);
            let idx = scope.fresh(name);
            if boolish {
                scope.bool_vars.insert(name.clone());
            } else {
                scope.bool_vars.remove(name);
            }
            Ok(Core::Let(idx, Box::new(ce), Box::new(compile_block(rest, scope, t, g, k)?)))
        }
        Stmt::Return(e) => compile_expr(e, scope, t),
        Stmt::ExprStmt(e) => {
            let ce = compile_expr(e, scope, t)?;
            let idx = scope.fresh_anon();
            Ok(Core::Let(idx, Box::new(ce), Box::new(compile_block(rest, scope, t, g, k)?)))
        }
        Stmt::If(cond, then, els) => {
            check_cond(cond, scope)?;
            let ccond = compile_expr(cond, scope, t)?;
            let arms = [(Vec::<String>::new(), then.as_slice()), (Vec::new(), els.as_slice())];
            let (cores, _binders, join) = compile_dispatch_arms(&arms, rest, scope, t, g, k)?;
            let producer = Core::If(Box::new(ccond), Box::new(cores[0].clone()), Box::new(cores[1].clone()));
            match join {
                Some(m) => bind_join_and_continue(producer, &m, rest, scope, t, g, k),
                None => Ok(producer),
            }
        }
        Stmt::While(cond, body) => {
            let (call, mutated) = compile_while(cond, body, scope, t, g)?;
            bind_join_and_continue(call, &mutated, rest, scope, t, g, k)
        }
        Stmt::For(var, bound, body, fold) => {
            let (call, mutated) = compile_for(var, bound, body, fold, scope, t, g)?;
            bind_join_and_continue(call, &mutated, rest, scope, t, g, k)
        }
        Stmt::Match(scrut, cases) => compile_match(scrut, cases, rest, scope, t, g, k),
    }
}

/// Bind an if/match join's or a loop's result: a bare value for one name,
/// a tuple otherwise (see `compile_dispatch_arms`).
fn bind_join_and_continue(
    producer: Core,
    names: &[String],
    rest: &[Stmt],
    scope: &mut Scope,
    t: &Tables,
    g: &mut Gen,
    k: &Cont,
) -> Result<Core, Diag> {
    if names.len() == 1 {
        let idx = scope.fresh(&names[0]);
        scope.bool_vars.remove(&names[0]);
        let core = compile_block(rest, scope, t, g, k)?;
        return Ok(Core::Let(idx, Box::new(producer), Box::new(core)));
    }
    bind_and_continue(producer, names, rest, scope, t, g, k)
}

/// Bind a `Tuple` producer's result into fresh indices for `names` (in
/// order), then continue compiling `rest` with those bindings visible.
fn bind_and_continue(
    producer: Core,
    names: &[String],
    rest: &[Stmt],
    scope: &mut Scope,
    t: &Tables,
    g: &mut Gen,
    k: &Cont,
) -> Result<Core, Diag> {
    let tup_idx = scope.fresh_anon();
    let mut idxs = Vec::new();
    for n in names {
        idxs.push(scope.fresh(n));
        scope.bool_vars.remove(n);
    }
    let mut core = compile_block(rest, scope, t, g, k)?;
    for (i, idx) in idxs.iter().enumerate().rev() {
        core = Core::Let(*idx, Box::new(Core::Proj(Box::new(Core::Var(tup_idx)), i)), Box::new(core));
    }
    Ok(Core::Let(tup_idx, Box::new(producer), Box::new(core)))
}

/// Compile the arms of an `if`/`match` statement that appears mid-block.
/// Each arm gets its own clone of `scope`, pre-bound with `binds` (pattern
/// binders, if any). An arm that returns on all its paths is compiled as a
/// plain terminal expression (its own `return`s produce the value
/// directly, `rest`/`k` are never reached for that path — anything after
/// is unreachable). An arm that *doesn't* always return instead inlines
/// the rest of the enclosing block (and the outer continuation) directly
/// as its own tail: this duplicates `rest`'s compiled `Core` once per such
/// arm rather than merging results through a shared tuple, which is
/// strictly more general (it makes early-return "guard clauses" mixed
/// with fallthrough arms — e.g. naive `fib`'s `if n < 2: return n` — work
/// for free) at the cost of some code-size duplication for `if`/`match`
/// statements where more than one arm falls through.
/// Returns the arm cores, their binder indices, and — when the arms are
/// compiled as a *join* — the names whose values every arm yields as a
/// tuple (the caller then binds them once and continues with `rest`).
///
/// Join form applies when no arm returns anywhere and every name any arm
/// assigns already exists in the enclosing scope: `rest` is then compiled
/// exactly once instead of once per arm (the SSA-style merge), which
/// keeps code size linear. Otherwise each fall-through arm inlines `rest`.
fn compile_dispatch_arms(
    arms: &[(Vec<String>, &[Stmt])],
    rest: &[Stmt],
    scope: &Scope,
    t: &Tables,
    g: &mut Gen,
    k: &Cont,
) -> Result<(Vec<Core>, Vec<Vec<u32>>, Option<Vec<String>>), Diag> {
    let mut assigned = BTreeSet::new();
    for (_, body) in arms {
        assigned.extend(assigned_names(body));
    }
    let joinable = !rest.is_empty()
        && arms.iter().all(|(binds, body)| {
            !contains_return(body) && binds.iter().all(|b| !assigned.contains(b))
        })
        && assigned.iter().all(|n| scope.vars.contains_key(n));
    let mutated: Vec<String> = assigned.into_iter().collect();
    let mut cores = Vec::new();
    let mut binder_idxs = Vec::new();
    for (binds, body) in arms {
        let mut s = scope.clone();
        let idxs: Vec<u32> = binds.iter().map(|n| s.fresh(n)).collect();
        let core = if joinable {
            let m = mutated.clone();
            compile_block(body, &mut s, t, g, &move |sc: &Scope, _g2: &mut Gen| {
                Ok(if m.len() == 1 {
                    Core::Var(sc.vars[&m[0]])
                } else {
                    Core::Tuple(m.iter().map(|n| Core::Var(sc.vars[n])).collect())
                })
            })?
        } else if always_returns(body) {
            compile_block(body, &mut s, t, g, &unreachable_tail)?
        } else {
            compile_block(body, &mut s, t, g, &|sc: &Scope, g2: &mut Gen| compile_block(rest, &mut sc.clone(), t, g2, k))?
        };
        cores.push(core);
        binder_idxs.push(idxs);
    }
    Ok((cores, binder_idxs, if joinable { Some(mutated) } else { None }))
}

fn compile_match(
    scrut: &Expr,
    cases: &[(Pat, Vec<Stmt>)],
    rest: &[Stmt],
    scope: &mut Scope,
    t: &Tables,
    g: &mut Gen,
    k: &Cont,
) -> Result<Core, Diag> {
    if cases.is_empty() {
        return Err(Diag::new(0, "match must have at least one case"));
    }
    let cscrut = compile_expr(scrut, scope, t)?;
    let is_int = cases.iter().all(|(p, _)| p.as_int_lit().is_some());
    if !is_int {
        check_exhaustive(cases, t)?;
    }
    let arm_specs: Vec<(Vec<String>, &[Stmt])> = cases.iter().map(|(p, b)| (p.binds.clone(), b.as_slice())).collect();
    let (cores, binder_idxs, join) = compile_dispatch_arms(&arm_specs, rest, scope, t, g, k)?;
    let producer = if is_int {
        let scrut_idx = scope.fresh_anon();
        let mut chain = Core::Ctor(UNREACHABLE_CTOR, vec![]);
        for ((p, _), core) in cases.iter().zip(cores.iter()).rev() {
            let v = p.as_int_lit().unwrap();
            chain = Core::If(
                Box::new(Core::Cmp(CmpOp::Eq, Box::new(Core::Var(scrut_idx)), Box::new(Core::Num(v)))),
                Box::new(core.clone()),
                Box::new(chain),
            );
        }
        Core::Let(scrut_idx, Box::new(cscrut), Box::new(chain))
    } else {
        let mut arms = Vec::new();
        for (i, (p, _)) in cases.iter().enumerate() {
            let &(cid, _) = t.ctor_table.get(&p.ctor).ok_or_else(|| Diag::new(0, format!("unknown constructor: {}", p.ctor)))?;
            arms.push((cid, binder_idxs[i].clone(), cores[i].clone()));
        }
        Core::Match(Box::new(cscrut), arms)
    };
    match join {
        Some(m) => bind_join_and_continue(producer, &m, rest, scope, t, g, k),
        None => Ok(producer),
    }
}

fn check_exhaustive(cases: &[(Pat, Vec<Stmt>)], t: &Tables) -> Result<(), Diag> {
    let first_ctor = &cases[0].0.ctor;
    let data_name = t.ctor_owner.get(first_ctor).cloned().ok_or_else(|| Diag::new(0, format!("unknown constructor: {}", first_ctor)))?;
    for (p, _) in cases {
        if t.ctor_owner.get(&p.ctor) != Some(&data_name) {
            return Err(Diag::new(0, format!("match mixes constructors from different @data types: '{}'", p.ctor)));
        }
    }
    let used: BTreeSet<&str> = cases.iter().map(|(p, _)| p.ctor.as_str()).collect();
    let declared = &t.data_ctors[&data_name];
    for c in declared {
        if !used.contains(c.as_str()) {
            return Err(Diag::new(0, format!("non-exhaustive match on '{}': missing case for constructor '{}'", data_name, c)));
        }
    }
    Ok(())
}

// ---- while / for -> fresh self-recursive helper functions ----

fn compile_while(cond: &Expr, body: &[Stmt], scope: &Scope, t: &Tables, g: &mut Gen) -> Result<(Core, Vec<String>), Diag> {
    check_cond(cond, scope)?;
    if contains_return(body) {
        return Err(Diag::new(0, "`return` inside a `while` body is not supported"));
    }
    let mutated_set = assigned_names(body);
    let mut free = BTreeSet::new();
    free_reads_expr(cond, &mut free);
    free_reads_stmts(body, &mut free);
    let params_all: Vec<String> = mutated_set
        .iter()
        .chain(free.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|n| scope.vars.contains_key(n))
        .collect();
    let mutated: Vec<String> = mutated_set.into_iter().filter(|n| params_all.contains(n)).collect();

    let helper_id = g.fresh_fn_id();
    let mut hscope = Scope::new();
    for p in &params_all {
        hscope.fresh(p);
    }
    let hcond = compile_expr(cond, &hscope, t)?;
    let params_cl = params_all.clone();
    let then_core = {
        let mut s = hscope.clone();
        compile_block(body, &mut s, t, g, &move |sc: &Scope, _g: &mut Gen| {
            Ok(Core::Call(helper_id, params_cl.iter().map(|p| Core::Var(sc.vars[p])).collect()))
        })?
    };
    // the loop state: a bare value for one variable, a tuple otherwise
    let else_core = if mutated.len() == 1 { Core::Var(hscope.vars[&mutated[0]]) } else { Core::Tuple(mutated.iter().map(|n| Core::Var(hscope.vars[n])).collect()) };
    let helper_body = Core::If(Box::new(hcond), Box::new(then_core), Box::new(else_core));
    let self_tail_rec = compute_self_tail_rec(helper_id, &helper_body);
    g.out_fns[helper_id as usize] = CoreFn {
        name: format!("__while{}", helper_id),
        arity: params_all.len(),
        body: helper_body,
        self_tail_rec,
        fold: None,
    };
    let call = Core::Call(helper_id, params_all.iter().map(|p| Core::Var(scope.vars[p])).collect());
    Ok((call, mutated))
}

fn compile_for(
    var: &str,
    bound: &Expr,
    body: &[Stmt],
    fold: &Option<AstFoldInfo>,
    scope: &mut Scope,
    t: &Tables,
    g: &mut Gen,
) -> Result<(Core, Vec<String>), Diag> {
    if contains_return(body) {
        return Err(Diag::new(0, "`return` inside a `for` body is not supported"));
    }
    let cbound = compile_expr(bound, scope, t)?;
    let bound_idx = scope.fresh_anon();

    let mutated_set: BTreeSet<String> = assigned_names(body).into_iter().filter(|n| n != var).collect();
    let mut free = BTreeSet::new();
    free_reads_stmts(body, &mut free);
    free.remove(var);
    let extra: Vec<String> = mutated_set
        .iter()
        .chain(free.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|n| scope.vars.contains_key(n))
        .collect();
    let mutated: Vec<String> = mutated_set.into_iter().filter(|n| extra.contains(n)).collect();

    let helper_id = g.fresh_fn_id();
    let mut hscope = Scope::new();
    let v_idx = hscope.fresh(var);
    let bnd_idx = hscope.fresh("__bound");
    for p in &extra {
        hscope.fresh(p);
    }
    let extra_cl = extra.clone();
    let then_core = {
        let mut s = hscope.clone();
        compile_block(body, &mut s, t, g, &move |sc: &Scope, _g: &mut Gen| {
            let mut args = vec![Core::Op2(BinOp::Add, Box::new(Core::Var(v_idx)), Box::new(Core::Num(1))), Core::Var(bnd_idx)];
            args.extend(extra_cl.iter().map(|p| Core::Var(sc.vars[p])));
            Ok(Core::Call(helper_id, args))
        })?
    };
    // the loop state: a bare value for one variable, a tuple otherwise
    let else_core = if mutated.len() == 1 { Core::Var(hscope.vars[&mutated[0]]) } else { Core::Tuple(mutated.iter().map(|n| Core::Var(hscope.vars[n])).collect()) };
    let hcond = Core::Cmp(CmpOp::Lt, Box::new(Core::Var(v_idx)), Box::new(Core::Var(bnd_idx)));
    let helper_body = Core::If(Box::new(hcond), Box::new(then_core), Box::new(else_core));
    let self_tail_rec = compute_self_tail_rec(helper_id, &helper_body);
    let core_fold = match fold {
        None => None,
        Some(fi) => Some(core::FoldInfo { combiner: convert_combiner(&fi.combiner, &t.fn_table)?, proven: fi.proven }),
    };
    g.out_fns[helper_id as usize] = CoreFn {
        name: format!("__for{}", helper_id),
        arity: 2 + extra.len(),
        body: helper_body,
        self_tail_rec,
        fold: core_fold,
    };

    let mut call_args = vec![Core::Num(0), Core::Var(bound_idx)];
    call_args.extend(extra.iter().map(|p| Core::Var(scope.vars[p])));
    let call = Core::Let(bound_idx, Box::new(cbound), Box::new(Core::Call(helper_id, call_args)));
    Ok((call, mutated))
}

fn convert_combiner(c: &AstCombiner, fn_table: &HashMap<String, FnId>) -> Result<Combiner, Diag> {
    Ok(match c {
        AstCombiner::WrapAdd => Combiner::WrapAdd,
        AstCombiner::TupleWrapAdd(n) => Combiner::TupleWrapAdd(*n),
        AstCombiner::WrapAdd32 => Combiner::WrapAdd32,
        AstCombiner::TupleWrapAdd32(n) => Combiner::TupleWrapAdd32(*n),
        AstCombiner::Fn(name) => Combiner::Fn(
            *fn_table.get(name).ok_or_else(|| Diag::new(0, format!("unknown fold combiner function: {}", name)))?,
        ),
    })
}

// ---- top-level function compilation ----

fn compile_fn(f: &FnDef, t: &Tables, g: &mut Gen) -> Result<CoreFn, Diag> {
    if !always_returns(&f.body) {
        return Err(Diag::new(0, format!("function '{}' does not return on all control-flow paths", f.name)));
    }
    let mut scope = Scope::new();
    for p in &f.params {
        scope.fresh(p);
    }
    let body = compile_block(&f.body, &mut scope, t, g, &unreachable_tail)?;
    let fid = *t.fn_table.get(&f.name).unwrap();
    let self_tail_rec = compute_self_tail_rec(fid, &body);
    Ok(CoreFn { name: f.name.clone(), arity: f.params.len(), body, self_tail_rec, fold: None })
}

// ---- tail-call analysis ----

/// Every self call of `fid` in `body` is in tail position.
pub fn compute_self_tail_rec(fid: FnId, body: &Core) -> bool {
    let mut ok = true;
    walk_tail(fid, body, true, &mut ok);
    ok
}

fn walk_tail(fid: FnId, c: &Core, is_tail: bool, ok: &mut bool) {
    match c {
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => {}
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
            walk_tail(fid, a, false, ok);
            walk_tail(fid, b, false, ok);
        }
        Core::If(c1, then, els) => {
            walk_tail(fid, c1, false, ok);
            walk_tail(fid, then, is_tail, ok);
            walk_tail(fid, els, is_tail, ok);
        }
        Core::Let(_, rhs, body) => {
            walk_tail(fid, rhs, false, ok);
            walk_tail(fid, body, is_tail, ok);
        }
        Core::Call(callee, args) => {
            for a in args {
                walk_tail(fid, a, false, ok);
            }
            if *callee == fid && !is_tail {
                *ok = false;
            }
        }
        Core::Ctor(_, args) | Core::Tuple(args) | Core::Reuse(_, _, args) | Core::Prim(_, args) => {
            for a in args {
                walk_tail(fid, a, false, ok);
            }
        }
        Core::Match(scrut, arms) => {
            walk_tail(fid, scrut, false, ok);
            for (_, _, b) in arms {
                walk_tail(fid, b, is_tail, ok);
            }
        }
        Core::Proj(e, _) => walk_tail(fid, e, false, ok),
        Core::Lam(_, b) => walk_tail(fid, b, false, ok),
        Core::App(f, a) => {
            walk_tail(fid, f, false, ok);
            walk_tail(fid, a, false, ok);
        }
    }
}
