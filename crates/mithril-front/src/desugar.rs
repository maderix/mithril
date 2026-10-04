//! Desugars the surface `Module` (statements, mutation, loops) into the
//! purely-functional `Core` IR: `while`/`for` become fresh self-recursive
//! top-level functions; `if`/`match` become `Core::If`/`Core::Match` with
//! their non-returning arms merged via a live-variable tuple (SSA-style).
//!
//! The surface `Expr`/`Stmt` types carry no line, so a diagnostic raised
//! here reports the line of the function's `def`.

use crate::ast::{BinOp, BoolOp, CmpOp, Expr, FnDef, FoldInfo, Module, Pat, Stmt};
use crate::core::{Core, CoreFn, CoreModule, CtorId, FnId, UNREACHABLE_CTOR};
use crate::Diag;
use std::collections::{BTreeSet, HashMap};

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
    // a `return` inside a loop: the loop's helper returns one of two
    // constructors, its state (it ended) or the function's value
    let loop_ctors = m.fns.iter().any(|f| loop_returns(&f.body, false)).then(|| {
        let at = ctors_flat.len() as u32;
        ctors_flat.push(("__LoopExit".into(), 1));
        ctors_flat.push(("__LoopReturn".into(), 1));
        (at, at + 1)
    });
    let t = Tables { fn_table, fn_arity, ctor_table, ctor_owner, data_ctors, loop_ctors };
    let mut g = Gen { out_fns: vec![placeholder(); m.fns.len()] };
    for (i, f) in m.fns.iter().enumerate() {
        let cf = compile_fn(f, &t, &mut g).map_err(|d| {
            let line = if d.line == 0 { m.lines.get(&f.name).copied().unwrap_or(0) } else { d.line };
            let msg = if d.msg.starts_with("in '") { d.msg } else { format!("in '{}': {}", f.name, d.msg) };
            Diag::new(line, msg)
        })?;
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
    /// (exit, return) constructors when some loop returns from its function
    loop_ctors: Option<(CtorId, CtorId)>,
}

/// A `return` inside a loop body (at any depth) in `stmts`.
fn loop_returns(stmts: &[Stmt], in_loop: bool) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Return(_) => in_loop,
        Stmt::While(_, b) | Stmt::For(_, _, b, _) => loop_returns(b, true),
        _ => s.parts().1.into_iter().any(|b| loop_returns(b, in_loop)),
    })
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
/// and the innermost loop being compiled.
#[derive(Clone, Default)]
struct Scope {
    vars: HashMap<String, u32>,
    next_idx: u32,
    lp: Option<std::rc::Rc<LoopCtx>>,
}
impl Scope {
    fn fresh(&mut self, name: &str) -> u32 {
        let i = self.fresh_anon();
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

/// The loop whose helper function a body is compiled into: what `continue`
/// calls, what `break` returns and whether exits are tagged (the body
/// returns from the function: see `Tables::loop_ctors`).
struct LoopCtx {
    id: FnId,
    /// a `for`: its counter and bound, the first two arguments
    counter: Option<(String, String)>,
    /// the rest of the arguments, by name
    params: Vec<String>,
    /// the state the loop yields when it ends
    mutated: Vec<String>,
    tagged: Option<(CtorId, CtorId)>,
}

impl LoopCtx {
    /// The next iteration from the names in scope `s`.
    fn next(&self, s: &Scope) -> Core {
        let mut args = Vec::new();
        if let Some((ctr, bound)) = &self.counter {
            args.push(Core::Op2(BinOp::Add, Box::new(Core::Var(s.vars[ctr])), Box::new(Core::Num(1))));
            args.push(Core::Var(s.vars[bound]));
        }
        args.extend(self.params.iter().map(|p| Core::Var(s.vars[p])));
        Core::Call(self.id, args)
    }

    /// The loop's result when it ends here.
    fn exit(&self, s: &Scope) -> Core {
        let st = state_value(&self.mutated, s);
        match self.tagged {
            Some((exit, _)) => Core::Ctor(exit, vec![st]),
            None => st,
        }
    }
}

// ---- static analysis over surface statements ----

/// One summary of the assignments and control flow in a surface block.
/// `returns` deliberately describes its last statement, as the existing
/// return check does; loops and integer matches may always fall through.
#[derive(Default)]
struct BlockFacts {
    assigned: BTreeSet<String>,
    surely: BTreeSet<String>,
    /// the block ends in a `return`, `break` or `continue` on every path
    returns: bool,
    /// a `return` anywhere, also inside nested loops
    has_return: bool,
    /// a `break` or `continue` of the enclosing loop (not of a nested one)
    has_jump: bool,
    straight: bool,
}

impl BlockFacts {
    fn of(stmts: &[Stmt]) -> Self {
        let mut out = Self { straight: true, ..Self::default() };
        for s in stmts {
            let arms: Vec<(Vec<String>, Self)> = match s {
                Stmt::If(_, t, e) => vec![(vec![], Self::of(t)), (vec![], Self::of(e))],
                Stmt::Match(_, cases) => cases.iter().map(|(p, b)| (p.binds.clone(), Self::of(b))).collect(),
                Stmt::While(_, b) | Stmt::For(_, _, b, _) => vec![(vec![], Self::of(b))],
                _ => vec![],
            };
            let exhaustive = matches!(s, Stmt::If(..))
                || matches!(s, Stmt::Match(_, cases) if cases.iter().all(|(p, _)| p.as_int_lit().is_none()));
            let jump = matches!(s, Stmt::Break | Stmt::Continue);
            out.returns = matches!(s, Stmt::Return(_)) || jump || (exhaustive && !arms.is_empty() && arms.iter().all(|(_, f)| f.returns));
            out.has_return |= matches!(s, Stmt::Return(_)) || arms.iter().any(|(_, f)| f.has_return);
            let is_loop = matches!(s, Stmt::While(..) | Stmt::For(..));
            out.has_jump |= jump || (!is_loop && arms.iter().any(|(_, f)| f.has_jump));
            out.straight &= matches!(s, Stmt::Assign(..) | Stmt::ExprStmt(_) | Stmt::Return(_));
            if let Stmt::Assign(n, _) = s {
                out.surely.insert(n.clone());
            }
            if exhaustive {
                out.surely.extend(Self::fallthrough(arms.iter().map(|(bs, f)| (bs.as_slice(), f))));
            }
            if let Stmt::Assign(n, _) | Stmt::For(n, _, _, _) = s {
                out.assigned.insert(n.clone());
            }
            for (_, f) in arms {
                out.assigned.extend(f.assigned);
            }
        }
        out
    }

    /// Pattern binders and assignments common to every non-returning arm.
    fn fallthrough<'a>(arms: impl Iterator<Item = (&'a [String], &'a Self)>) -> BTreeSet<String> {
        let mut common: Option<BTreeSet<String>> = None;
        for (binds, f) in arms.filter(|(_, f)| !f.returns) {
            let mut defined = f.surely.clone();
            defined.extend(binds.iter().cloned());
            common = Some(match common {
                None => defined,
                Some(c) => c.intersection(&defined).cloned().collect(),
            });
        }
        common.unwrap_or_default()
    }
}

/// A `break` or `continue` of this loop in `body` (not of a nested loop).
pub fn has_jump(body: &[Stmt]) -> bool {
    BlockFacts::of(body).has_jump
}

/// Names assigned anywhere in a block; a for induction variable counts.
pub fn assigned_names(stmts: &[Stmt]) -> BTreeSet<String> {
    BlockFacts::of(stmts).assigned
}

/// Names `e` reads that it does not bind (a lambda's parameters are bound).
pub fn free_reads_expr(e: &Expr, out: &mut BTreeSet<String>) {
    match e {
        Expr::Var(n) | Expr::Call(n, _) => { out.insert(n.clone()); }
        Expr::Lambda(params, body) => {
            let mut inner = BTreeSet::new();
            free_reads_expr(body, &mut inner);
            for p in params { inner.remove(p); }
            out.extend(inner);
            return;
        }
        _ => {}
    }
    for child in e.kids() { free_reads_expr(child, out); }
}

pub fn free_reads_stmts(stmts: &[Stmt], out: &mut BTreeSet<String>) {
    for st in stmts {
        let (e, blocks) = st.parts();
        free_reads_expr(e, out);
        for block in blocks { free_reads_stmts(block, out); }
    }
}

// ---- expression compilation ----

/// Curry `body` over `ps`, outermost first.
fn lams(ps: Vec<u32>, body: Core) -> Core {
    ps.into_iter().rev().fold(body, |b, p| Core::Lam(p, Box::new(b)))
}

fn compile_expr(e: &Expr, scope: &Scope, t: &Tables) -> Result<Core, Diag> {
    let c = |e: &Expr| compile_expr(e, scope, t).map(Box::new);
    let num = |n| Box::new(Core::Num(n));
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
                    let call = Core::Call(fid, params.iter().map(|p| Core::Var(*p)).collect());
                    Ok(lams(params, call))
                }
                None => Err(e),
            },
        },
        Expr::Bin(op, a, b) => Ok(Core::Op2(*op, c(a)?, c(b)?)),
        Expr::Cmp(op, a, b) => Ok(Core::Cmp(*op, c(a)?, c(b)?)),
        Expr::Bool2(BoolOp::And, a, b) => Ok(Core::If(c(a)?, c(b)?, num(0))),
        // Python's `a or b` is `a` when `a` is true
        Expr::Bool2(BoolOp::Or, a, b) => {
            let mut s = scope.clone();
            let v = s.fresh_anon();
            Ok(Core::Let(v, c(a)?, Box::new(Core::If(Box::new(Core::Var(v)), Box::new(Core::Var(v)), c(b)?))))
        }
        Expr::Not(a) => Ok(Core::If(c(a)?, num(0), num(1))),
        // on ints; `infer` rewrites a float negation before desugaring
        Expr::Neg(a) => Ok(Core::Op2(BinOp::Sub, num(0), c(a)?)),
        Expr::IfExp(cond, then, els) => Ok(Core::If(c(cond)?, c(then)?, c(els)?)),
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
            Expr::Int(n) if *n >= 0 => Ok(Core::Proj(c(base)?, *n as usize)),
            _ => Err(Diag::new(0, "tuple index must be a non-negative integer literal")),
        },
        Expr::Lambda(params, body) => {
            // curried: each parameter is a fresh variable of an inner scope
            let mut s = scope.clone();
            let idxs: Vec<u32> = params.iter().map(|p| s.fresh(p)).collect();
            Ok(lams(idxs, compile_expr(body, &s, t)?))
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
            let idx = scope.fresh(name);
            Ok(Core::Let(idx, Box::new(ce), Box::new(compile_block(rest, scope, t, g, k)?)))
        }
        // inside a loop's helper the function's value leaves tagged
        Stmt::Return(e) => match (&scope.lp, t.loop_ctors) {
            (Some(_), Some((_, ret))) => Ok(Core::Ctor(ret, vec![compile_expr(e, scope, t)?])),
            _ => compile_expr(e, scope, t),
        },
        Stmt::Break | Stmt::Continue => {
            let word = if matches!(head, Stmt::Break) { "break" } else { "continue" };
            let lp = scope.lp.clone().ok_or_else(|| Diag::new(0, format!("`{word}` outside a loop")))?;
            Ok(if matches!(head, Stmt::Break) { lp.exit(scope) } else { lp.next(scope) })
        }
        Stmt::ExprStmt(e) => {
            let ce = compile_expr(e, scope, t)?;
            let idx = scope.fresh_anon();
            Ok(Core::Let(idx, Box::new(ce), Box::new(compile_block(rest, scope, t, g, k)?)))
        }
        Stmt::If(cond, then, els) => {
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
            continue_after_loop(call, &mutated, BlockFacts::of(body).has_return, rest, scope, t, g, k)
        }
        Stmt::For(var, bound, body, fold) => {
            let (call, mutated) = compile_for(var, bound, body, fold, scope, t, g)?;
            continue_after_loop(call, &mutated, BlockFacts::of(body).has_return, rest, scope, t, g, k)
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
    let tup = (names.len() != 1).then(|| scope.fresh_anon());
    let idxs: Vec<u32> = names.iter().map(|n| scope.fresh(n)).collect();
    let mut core = compile_block(rest, scope, t, g, k)?;
    let Some(tup) = tup else { return Ok(Core::Let(idxs[0], Box::new(producer), Box::new(core))) };
    for (i, idx) in idxs.iter().enumerate().rev() {
        core = Core::Let(*idx, Box::new(Core::Proj(Box::new(Core::Var(tup)), i)), Box::new(core));
    }
    Ok(Core::Let(tup, Box::new(producer), Box::new(core)))
}

/// After a loop: its state continues the block. A loop that can return
/// from the function yields a tagged value, and its return either is the
/// function's value or, inside an enclosing loop, leaves that loop too.
#[allow(clippy::too_many_arguments)]
fn continue_after_loop(
    call: Core,
    mutated: &[String],
    tagged: bool,
    rest: &[Stmt],
    scope: &mut Scope,
    t: &Tables,
    g: &mut Gen,
    k: &Cont,
) -> Result<Core, Diag> {
    let (true, Some((exit, ret))) = (tagged, t.loop_ctors) else {
        return bind_join_and_continue(call, mutated, rest, scope, t, g, k);
    };
    let (st, v) = (scope.fresh_anon(), scope.fresh_anon());
    let returned = if scope.lp.is_some() { Core::Ctor(ret, vec![Core::Var(v)]) } else { Core::Var(v) };
    let ended = bind_join_and_continue(Core::Var(st), mutated, rest, scope, t, g, k)?;
    Ok(Core::Match(Box::new(call), vec![(exit, vec![st], ended), (ret, vec![v], returned)]))
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
/// keeps code size linear.
///
/// Otherwise a fall-through arm inlines `rest` as its own tail, unless several
/// arms fall through and `rest` has control flow of its own: copying such a `rest`
/// into each arm doubles the code per statement (16 sequential ifs gave 22.9 MB of
/// Core). Those arms call a join function instead (see `compile_join`).
fn compile_dispatch_arms(
    arms: &[(Vec<String>, &[Stmt])],
    rest: &[Stmt],
    scope: &Scope,
    t: &Tables,
    g: &mut Gen,
    k: &Cont,
) -> Result<(Vec<Core>, Vec<Vec<u32>>, Option<Vec<String>>), Diag> {
    let facts: Vec<BlockFacts> = arms.iter().map(|(_, b)| BlockFacts::of(b)).collect();
    let assigned: BTreeSet<String> = facts.iter().flat_map(|f| f.assigned.iter().cloned()).collect();
    let joinable = !rest.is_empty()
        && arms.iter().zip(&facts).all(|((binds, _), f)| {
            !f.has_return && !f.has_jump && binds.iter().all(|b| !assigned.contains(b))
        })
        && assigned.iter().all(|n| scope.vars.contains_key(n));
    let mutated: Vec<String> = assigned.into_iter().collect();
    let falling = facts.iter().filter(|f| !f.returns).count();
    let rest_facts = BlockFacts::of(rest);
    let join = if !joinable && !rest.is_empty() && falling > 1 && !(rest_facts.straight && rest_facts.returns) {
        Some(compile_join(arms, &facts, rest, scope, t, g, k)?)
    } else {
        None
    };
    let mut cores = Vec::new();
    let mut binder_idxs = Vec::new();
    for ((binds, body), f) in arms.iter().zip(&facts) {
        let mut s = scope.clone();
        let idxs: Vec<u32> = binds.iter().map(|n| s.fresh(n)).collect();
        let core = if joinable {
            let m = mutated.clone();
            compile_block(body, &mut s, t, g, &move |sc: &Scope, _g2: &mut Gen| Ok(state_value(&m, sc)))?
        } else if f.returns {
            compile_block(body, &mut s, t, g, &unreachable_tail)?
        } else if let Some((id, names)) = &join {
            let call_join = |sc: &Scope, _: &mut Gen| {
                let args = names.iter().map(|n| Core::Var(sc.vars[n])).collect();
                Ok(Core::Call(*id, args))
            };
            compile_block(body, &mut s, t, g, &call_join)?
        } else {
            compile_block(body, &mut s, t, g, &|sc: &Scope, g2: &mut Gen| compile_block(rest, &mut sc.clone(), t, g2, k))?
        };
        cores.push(core);
        binder_idxs.push(idxs);
    }
    Ok((cores, binder_idxs, if joinable { Some(mutated) } else { None }))
}

/// The join function `__join<id>` of an if/match statement: `rest` compiled once,
/// over the names in scope after the arms (the enclosing ones and those every
/// fall-through arm defines). Returns its id and parameter names.
fn compile_join(
    arms: &[(Vec<String>, &[Stmt])],
    facts: &[BlockFacts],
    rest: &[Stmt],
    scope: &Scope,
    t: &Tables,
    g: &mut Gen,
    k: &Cont,
) -> Result<(FnId, Vec<String>), Diag> {
    let mut names: BTreeSet<String> = scope.vars.keys().cloned().collect();
    names.extend(BlockFacts::fallthrough(arms.iter().zip(facts).map(|((bs, _), f)| (bs.as_slice(), f))));
    let names: Vec<String> = names.into_iter().collect();

    let id = g.fresh_fn_id();
    let mut js = Scope::default();
    for n in &names {
        js.fresh(n);
    }
    // a `break` or `continue` in the rest still belongs to the enclosing loop
    js.lp = scope.lp.clone();
    let body = compile_block(rest, &mut js, t, g, k)?;
    g.out_fns[id as usize] = CoreFn {
        name: format!("__join{id}"),
        arity: names.len(),
        self_tail_rec: compute_self_tail_rec(id, &body),
        body,
        fold: None,
    };
    Ok((id, names))
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
    let mut arm_specs: Vec<(Vec<String>, &[Stmt])> = cases.iter().map(|(p, b)| (p.binds.clone(), b.as_slice())).collect();
    if is_int {
        // no case matches: the statement does nothing (Python)
        arm_specs.push((Vec::new(), &[]));
    }
    let (cores, binder_idxs, join) = compile_dispatch_arms(&arm_specs, rest, scope, t, g, k)?;
    let producer = if is_int {
        let scrut_idx = scope.fresh_anon();
        let mut chain = cores[cases.len()].clone();
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

/// A loop helper's parameters (names it mutates or reads that exist outside
/// it, sorted) and the mutated ones among them.
fn loop_state(mutated: BTreeSet<String>, free: &BTreeSet<String>, scope: &Scope) -> (Vec<String>, Vec<String>) {
    let params = mutated.union(free).filter(|n| scope.vars.contains_key(*n)).cloned().collect();
    (params, mutated.into_iter().filter(|n| scope.vars.contains_key(n)).collect())
}

/// The state `names` in scope `s`: a bare value for one name, a tuple otherwise.
fn state_value(names: &[String], s: &Scope) -> Core {
    let mut vs: Vec<Core> = names.iter().map(|n| Core::Var(s.vars[n])).collect();
    if vs.len() == 1 { vs.pop().unwrap() } else { Core::Tuple(vs) }
}

/// Both loop forms compile their body with a next-iteration continuation,
/// return the unchanged state on exit, and register the same tail helper.
fn compile_loop(
    id: FnId, kind: &str, mut scope: Scope, cond: Core, body: &[Stmt], mutated: &[String], fold: Option<FoldInfo>,
    t: &Tables, g: &mut Gen, next: &Cont,
) -> Result<(), Diag> {
    // Parameter indices and exit state precede the body's local bindings.
    let arity = scope.next_idx as usize;
    let exit = scope.lp.as_ref().map_or_else(|| state_value(mutated, &scope), |lp| lp.exit(&scope));
    let then = compile_block(body, &mut scope, t, g, next)?;
    let body = Core::If(Box::new(cond), Box::new(then), Box::new(exit));
    g.out_fns[id as usize] = CoreFn {
        name: format!("__{kind}{id}"),
        arity,
        self_tail_rec: compute_self_tail_rec(id, &body),
        body,
        fold,
    };
    Ok(())
}

fn compile_while(cond: &Expr, body: &[Stmt], scope: &Scope, t: &Tables, g: &mut Gen) -> Result<(Core, Vec<String>), Diag> {
    let facts = BlockFacts::of(body);
    let tagged = if facts.has_return { t.loop_ctors } else { None };
    let mut free = BTreeSet::new();
    free_reads_expr(cond, &mut free);
    free_reads_stmts(body, &mut free);
    let (params_all, mutated) = loop_state(facts.assigned, &free, scope);

    let helper_id = g.fresh_fn_id();
    let mut hscope = Scope::default();
    for p in &params_all {
        hscope.fresh(p);
    }
    let hcond = compile_expr(cond, &hscope, t)?;
    let lp = std::rc::Rc::new(LoopCtx { id: helper_id, counter: None, params: params_all.clone(), mutated: mutated.clone(), tagged });
    hscope.lp = Some(lp.clone());
    let next = move |sc: &Scope, _: &mut Gen| Ok(lp.next(sc));
    compile_loop(helper_id, "while", hscope, hcond, body, &mutated, None, t, g, &next)?;
    let call = Core::Call(helper_id, params_all.iter().map(|p| Core::Var(scope.vars[p])).collect());
    Ok((call, mutated))
}

fn compile_for(
    var: &str,
    bound: &Expr,
    body: &[Stmt],
    fold: &Option<FoldInfo>,
    scope: &mut Scope,
    t: &Tables,
    g: &mut Gen,
) -> Result<(Core, Vec<String>), Diag> {
    let facts = BlockFacts::of(body);
    let tagged = if facts.has_return { t.loop_ctors } else { None };
    // an early exit makes the loop sequential: no fold
    let fold = if facts.has_return || facts.has_jump { &None } else { fold };
    let cbound = compile_expr(bound, scope, t)?;
    let bound_idx = scope.fresh_anon();

    let mut free = BTreeSet::new();
    free_reads_stmts(body, &mut free);
    free.remove(var);
    let (extra, mutated) = loop_state(facts.assigned.into_iter().filter(|n| n != var).collect(), &free, scope);

    let helper_id = g.fresh_fn_id();
    let mut hscope = Scope::default();
    let counter = format!("$for{helper_id}");
    let bound_name = format!("$bound{helper_id}");
    let v_idx = hscope.fresh(var);
    // Immutable iterator state must survive lifted joins and assignments to var.
    hscope.vars.insert(counter.clone(), v_idx);
    let bnd_idx = hscope.fresh(&bound_name);
    for p in &extra {
        hscope.fresh(p);
    }
    let lp = std::rc::Rc::new(LoopCtx {
        id: helper_id,
        counter: Some((counter.clone(), bound_name.clone())),
        params: extra.clone(),
        mutated: mutated.clone(),
        tagged,
    });
    hscope.lp = Some(lp.clone());
    let next = move |sc: &Scope, _: &mut Gen| Ok(lp.next(sc));
    let hcond = Core::Cmp(CmpOp::Lt, Box::new(Core::Var(v_idx)), Box::new(Core::Var(bnd_idx)));
    compile_loop(helper_id, "for", hscope, hcond, body, &mutated, fold.clone(), t, g, &next)?;

    let mut call_args = vec![Core::Num(0), Core::Var(bound_idx)];
    call_args.extend(extra.iter().map(|p| Core::Var(scope.vars[p])));
    let call = Core::Let(bound_idx, Box::new(cbound), Box::new(Core::Call(helper_id, call_args)));
    Ok((call, mutated))
}

// ---- top-level function compilation ----

fn compile_fn(f: &FnDef, t: &Tables, g: &mut Gen) -> Result<CoreFn, Diag> {
    if !BlockFacts::of(&f.body).returns {
        return Err(Diag::new(0, format!("function '{}' does not return on all control-flow paths", f.name)));
    }
    let mut scope = Scope::default();
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
    // the tail passes into every child but the first of If, Let and Match
    fn ok(fid: FnId, c: &Core, tail: bool) -> bool {
        let passes = matches!(c, Core::If(..) | Core::Let(..) | Core::Match(..));
        !matches!(c, Core::Call(g, _) if *g == fid && !tail)
            && c.kids().into_iter().enumerate().all(|(i, k)| ok(fid, k, tail && passes && i > 0))
    }
    ok(fid, body, true)
}
