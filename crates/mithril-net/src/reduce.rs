//! Compile-time reducer (fuel-bounded, deterministic FIFO worklist) and
//! readback of fully-reduced values.

use crate::rules::{flo_bits, process};
use crate::{Mode, NetProg, CTAG_TUPLE, CTAG_UNREACHABLE, EMPTY};
use mithril_core::net::Net;
use mithril_core::port::{Port, Tag};
use mithril_front::core::{Core, CoreModule, Val};
use std::collections::{HashMap, HashSet};

/// Run interaction rules on the net's redex worklist, in FIFO order, until
/// quiescence or until `fuel` rewrites have been performed (pure wiring
/// steps are free but only happen while fuel remains, so `fuel == 0`
/// leaves the net untouched). Unprocessed redexes stay queued, so a
/// fuel-starved reduction can be resumed by calling `reduce` again.
/// Returns the number of rewrites performed.
pub fn reduce(net: &mut Net, m: &CoreModule, fuel: u64) -> u64 {
    let prog = NetProg::new(m);
    let mut done = 0u64;
    let mut i = 0usize;
    while done < fuel && i < net.redexes.len() {
        let (a, b) = net.redexes[i];
        i += 1;
        done += process(net, &prog, a, b);
    }
    net.redexes.drain(..i);
    done
}

/// Follow filled wires without consuming them (readback is `&Net`).
fn peek(net: &Net, mut p: Port) -> Port {
    while p.tag() == Tag::Var {
        let c = net.cell(p.payload() as u32);
        if c[0] == EMPTY.0 {
            return p;
        }
        p = Port(c[0]);
    }
    p
}

/// Read a fully-reduced value: NUM -> `Val::I` (sign-extended i56), FLO
/// cell -> `Val::F`, CON chains -> `Val::C`/`Val::T`. Anything else (an
/// unfilled wire, a residual agent, the unreachable-match sentinel) is not
/// a value: `None`.
pub fn readback(net: &Net, root: Port) -> Option<Val> {
    let p = peek(net, root);
    match p.tag() {
        Tag::Num => Some(Val::I(p.as_i64())),
        Tag::Flo => Some(Val::F(f64::from_bits(flo_bits(net.cell(p.payload() as u32))))),
        Tag::Con => read_con(net, p),
        _ => None,
    }
}

fn read_con(net: &Net, mut p: Port) -> Option<Val> {
    let ctag = p.con_tag();
    if ctag == CTAG_UNREACHABLE {
        return None;
    }
    let mut fields = Vec::new();
    loop {
        let n = p.con_arity();
        let a = p.con_addr() as u32;
        match n {
            0 => break,
            1 => {
                fields.push(readback(net, Port(net.cell(a)[0]))?);
                break;
            }
            2 => {
                let c = net.cell(a);
                fields.push(readback(net, Port(c[0]))?);
                fields.push(readback(net, Port(c[1]))?);
                break;
            }
            _ => {
                let c = net.cell(a);
                fields.push(readback(net, Port(c[0]))?);
                p = Port(c[1]);
                if p.tag() != Tag::Con {
                    return None;
                }
            }
        }
    }
    if ctag == CTAG_TUPLE {
        Some(Val::T(fields))
    } else {
        Some(Val::C(ctag as u32, fields))
    }
}

/// Per-function specialization report.
#[derive(Clone, Debug, Default)]
pub struct SpecReport {
    pub name: String,
    pub rewrites: u64,
    /// calls the policy kept, ops that could not fold (opaque or failing)
    pub calls_kept: usize,
    pub ops_kept: usize,
    /// fully static calls to recursive functions evaluated to their value
    pub calls_evaluated: usize,
    pub size_before: usize,
    pub size_after: usize,
}

/// Rewrite budget for evaluating one fully static call in a scratch net,
/// and the largest value (in cells) worth embedding in the program.
const SPEC_CALL_FUEL: u64 = 200_000;
const SPEC_VALUE_CELLS: usize = 4096;
/// Budgets for unfolding a partially static call: rewrites, net growth in
/// agents (ops, branches, data: what becomes code; wires are free),
/// nesting depth.
const SPEC_UNFOLD_FUEL: u64 = 50_000;
const SPEC_UNFOLD_AGENTS: usize = 2000;
/// Nesting bound of speculative unfolds (an unfolded body's own calls are
/// unfolded in turn: a loop with a static bound unrolls as a chain).
const SPEC_DEPTH: u32 = 256;
/// Total speculative rewrites spent (accepted or not) per function.
const SPEC_TOTAL_FUEL: u64 = 400_000;

/// Specialize every function by the interaction rules. A function's body
/// is built as a net over unknown parameters and reduced to quiescence:
/// the static gate parks every rule on an unknown value, everything else
/// (constant ops, small acyclic calls, branches and matches on known
/// values, sharing) fires. A call to a recursive function whose arguments
/// are all known is evaluated in a scratch net under a budget and replaced
/// by its value; other calls stay calls. Then every branch parked on an
/// unknown value has its arms instantiated (each in its own scope frame,
/// over fresh unknown wires for pattern binders) and reduced the same way,
/// to a fixpoint, so code under runtime branches is specialized too. The
/// residual net is read back as the function's new body.
///
/// `MITHRIL_NET_TRACE=1` narrates the process on stderr.
pub fn specialize(m: &CoreModule, fuel: u64) -> (CoreModule, Vec<SpecReport>) {
    let mut prog = NetProg::new(m);
    prog.mode = Mode::Specialize;
    let eval_prog = NetProg::new(m);
    let mut out = m.clone();
    let mut reports = Vec::new();
    if trace() {
        for (i, f) in m.fns.iter().enumerate() {
            eprintln!("fn {i} = {} (arity {}, inline {})", f.name, f.arity, prog.inline[i]);
        }
    }
    for (fid, f) in m.fns.iter().enumerate() {
        let mut net = Net::new();
        let params = crate::build::build_fn(&mut net, &prog, fid);
        let free: Vec<(Port, u32)> = params.iter().enumerate().map(|(i, p)| (*p, i as u32)).collect();
        let mut fx = Fx { net, free, arms: HashMap::new(), dup_frame: HashMap::new(), nframes: 1, next_var: max_var(&f.body).max(f.arity as u32) + 1 };
        let mut done = 0u64;
        let mut evaluated = 0usize;
        let mut memo = Spec::default();
        let ok = settle(&mut fx, &prog, &eval_prog, fuel, &mut done, &mut evaluated, &mut memo, 0, 0, &HashSet::new(), &HashSet::new());
        assert!(ok, "ICE: specialization of `{}` ran out of fuel ({} rewrites)", f.name, done);
        if std::env::var_os("MITHRIL_NET_DUMP").is_some() {
            eprintln!("== {} free wires {:?}\n{}", f.name, fx.free, fx.net.dump());
            eprintln!("residual: {:?}", fx.net.residual.iter().map(|(a, b)| (a.tag(), a.payload(), b.tag(), b.payload())).collect::<Vec<_>>());
        }
        let calls_kept = fx.net.residual.iter().filter(|(a, _)| a.tag() == Tag::Ref).count();
        let ops_kept = fx.net.residual.len() - calls_kept;
        let body = {
            let mut rd = crate::residual::Reader::new(&fx.net, &prog, &ROOTS, &fx.free, fx.next_var, &fx.dup_frame, &fx.arms);
            rd.read_frame(0, crate::root_port())
        };
        if trace() {
            eprintln!("{}: {} rewrites, {} calls kept, {} ops kept, {} calls evaluated, size {} -> {}", f.name, done, calls_kept, ops_kept, evaluated, crate::core_size(&f.body), crate::core_size(&body));
        }
        reports.push(SpecReport {
            name: f.name.clone(),
            rewrites: done,
            calls_kept,
            ops_kept,
            calls_evaluated: evaluated,
            size_before: crate::core_size(&f.body),
            size_after: crate::core_size(&body),
        });
        out.fns[fid].self_tail_rec = mithril_front::desugar::compute_self_tail_rec(fid as u32, &body);
        out.fns[fid].body = body;
    }
    (out, reports)
}

/// The root wire, the only root of a function's net.
const ROOTS: [Port; 1] = [Port(0)];

/// The state of one function's specialization: its net and everything
/// the readback needs to interpret it. Cloned to speculate.
#[derive(Clone)]
struct Fx {
    net: Net,
    /// unknown input wires (parameters, pattern binders) and their variables
    free: Vec<(Port, u32)>,
    /// instantiated arms of parked branches: (agent cell, arm index)
    arms: HashMap<(u32, usize), crate::residual::ArmInfo>,
    /// the scope frame each Dup cell belongs to
    dup_frame: HashMap<u32, usize>,
    nframes: usize,
    next_var: u32,
}

/// Specialize to a fixpoint: reduce and settle the calls (`run`), then
/// instantiate the arms of every branch parked on an unknown value, each
/// in a fresh scope frame, and reduce those too, until nothing is parked
/// without its arms. `frame` is the scope the work at hand belongs to;
/// `skip` are parked agents left to an enclosing settle (inside a
/// speculation, only what the speculated body parked is instantiated),
/// `old_refs` the residual calls that were there before it: a speculation
/// that leaves a new call is rejected as soon as the call is seen.
/// `false` on fuel-out or rejection.
#[allow(clippy::too_many_arguments)]
fn settle(
    fx: &mut Fx,
    prog: &NetProg,
    eval_prog: &NetProg,
    fuel: u64,
    done: &mut u64,
    evaluated: &mut usize,
    memo: &mut Spec,
    depth: u32,
    frame: usize,
    skip: &HashSet<u32>,
    old_refs: &HashSet<u64>,
) -> bool {
    let new_call = |fx: &Fx| depth > 0 && fx.net.residual.iter().any(|(a, _)| a.tag() == Tag::Ref && !old_refs.contains(&a.0));
    if !run(fx, prog, eval_prog, fuel, done, evaluated, memo, depth, frame) || new_call(fx) {
        return false;
    }
    assign_dups(&fx.net, &ROOTS, &fx.free, &mut fx.dup_frame, frame);
    loop {
        let ix = crate::residual::scan(&fx.net, &ROOTS, &fx.free, &fx.net.residual);
        let todo: Vec<(u32, bool)> = ix.parked.iter().copied().filter(|(a, _)| !skip.contains(a) && !fx.arms.contains_key(&(*a, 0))).collect();
        if trace() {
            eprintln!("{}settle: parked {} todo {} cells {} residual {} rewrites {}", "  ".repeat(depth as usize), ix.parked.len(), todo.len(), fx.net.cells.len(), fx.net.residual.len(), done);
        }
        if todo.is_empty() {
            return true;
        }
        for (agent, is_match) in todo {
            let slots: Vec<(u32, usize)> = if is_match {
                // arm list cells: (cell, slot 0)
                let mut head = Port(fx.net.cell(agent)[1]);
                let mut v = Vec::new();
                while head != EMPTY {
                    v.push((head.payload() as u32, 0));
                    head = Port(fx.net.cell(head.payload() as u32)[1]);
                }
                v
            } else {
                let s2 = Port(fx.net.cell(agent)[1]).payload() as u32;
                vec![(s2, 0), (s2, 1)]
            };
            for (i, (cell, slot)) in slots.into_iter().enumerate() {
                let r = Port(fx.net.cell(cell)[slot]);
                assert_eq!(r.tag(), Tag::Ref, "ICE: parked arm slot holds {:?}", r.tag());
                let entry = crate::ref_entry(r) as usize;
                let caps = crate::list_collect(&mut fx.net, crate::ref_head(r));
                let e = &prog.entries[entry];
                // pattern binders get fresh variables of this function
                // (the arm's own numbering belongs to the callee it was
                // inlined from and may collide with the readback's)
                let nb = e.params.len() - caps.len();
                let binders: Vec<u32> = (0..nb).map(|_| { fx.next_var += 1; fx.next_var - 1 }).collect();
                let mut args: Vec<Port> = Vec::with_capacity(e.params.len());
                for b in &binders {
                    let w = crate::wire(&mut fx.net);
                    fx.free.push((w, *b));
                    args.push(w);
                }
                args.extend(caps);
                let ret = crate::wire(&mut fx.net);
                fx.net.set(cell, slot, ret);
                let f = fx.nframes;
                fx.nframes += 1;
                fx.arms.insert((agent, i), crate::residual::ArmInfo { binders, frame: f });
                if trace() {
                    eprintln!("{}arm {i} of agent {agent} -> entry {entry}", "  ".repeat(depth as usize + 1));
                }
                crate::build::instantiate(&mut fx.net, prog, entry, args, ret);
                if !run(fx, prog, eval_prog, fuel, done, evaluated, memo, depth, f) || new_call(fx) {
                    return false;
                }
                assign_dups(&fx.net, &ROOTS, &fx.free, &mut fx.dup_frame, f);
            }
        }
    }
}

/// Reduce to quiescence, then settle the calls the policy did not unfold:
/// one whose arguments are all known is evaluated in a scratch net and
/// replaced by its value; one with some known arguments is unfolded
/// speculatively (accepted when its control is static within budget); the
/// rest stay residual. Loops until nothing changes. `false` on fuel-out.
#[allow(clippy::too_many_arguments)]
fn run(
    fx: &mut Fx,
    prog: &NetProg,
    eval_prog: &NetProg,
    fuel: u64,
    done: &mut u64,
    evaluated: &mut usize,
    memo: &mut Spec,
    depth: u32,
    frame: usize,
) -> bool {
    loop {
        let net = &mut fx.net;
        let mut i = 0usize;
        while *done < fuel && i < net.redexes.len() {
            let (a, b) = net.redexes[i];
            i += 1;
            *done += process(net, prog, a, b);
        }
        net.redexes.drain(..i);
        if !net.redexes.is_empty() || (memo.cap != usize::MAX && agents(net) > memo.cap) {
            return false;
        }
        // settle in place: the list stays complete on the net while a
        // call is speculated on (its scan must see every parked agent)
        let mut progress = false;
        let mut i = 0usize;
        while i < fx.net.residual.len() {
            let (a, b) = fx.net.residual[i];
            if a.tag() == Tag::Ref {
                if args_known(&fx.net, crate::ref_head(a)) {
                    let key = call_key(&fx.net, a);
                    if memo.memo.get(&key) != Some(&false) {
                        if let Some(v) = evaluate_call(&mut fx.net, eval_prog, a) {
                            fx.net.residual.swap_remove(i);
                            crate::rules::link(&mut fx.net, v, b);
                            *evaluated += 1;
                            progress = true;
                            continue;
                        }
                        memo.memo.insert(key, false);
                    }
                } else if depth < SPEC_DEPTH {
                    // a failed partial unfold is about the callee's control
                    // under this pattern of known arguments, not the values:
                    // memoized by pattern, bounded by a per-function budget
                    let pat = pattern_key(&fx.net, a);
                    if pat.contains('k') && memo.memo.get(&pat) != Some(&false) && memo.spent < SPEC_TOTAL_FUEL {
                        if unfold_call(fx, prog, eval_prog, i, fuel, done, evaluated, memo, depth, frame) {
                            progress = true;
                            continue;
                        }
                        memo.memo.insert(pat, false);
                    }
                }
            }
            i += 1;
        }
        if !progress {
            return true;
        }
    }
}

/// `MITHRIL_NET_TRACE=1`: narrate specialization on stderr.
fn trace() -> bool {
    std::env::var_os("MITHRIL_NET_TRACE").is_some()
}

/// Per-function speculation state: failed attempts (by call key or by
/// argument pattern) and the rewrites spent speculating.
struct Spec {
    memo: HashMap<String, bool>,
    spent: u64,
    /// live-cell ceiling of the speculation in progress (a nested unfold
    /// inherits the outer one's budget: a chain stops growing early)
    cap: usize,
}

impl Default for Spec {
    fn default() -> Spec {
        Spec { memo: HashMap::new(), spent: 0, cap: usize::MAX }
    }
}

/// A memo key for a partial unfold: the entry and which arguments are known.
fn pattern_key(net: &Net, r: Port) -> String {
    let pat: String = crate::list_items(net, crate::ref_head(r))
        .into_iter()
        .map(|a| if readback(net, a).is_some() { 'k' } else { '?' })
        .collect();
    format!("#{}({})", crate::ref_entry(r), pat)
}

/// A memo key for a call: its entry and which arguments are known (with
/// their values), so a failed attempt is not repeated at every pass.
fn call_key(net: &Net, r: Port) -> String {
    let args: Vec<String> = crate::list_items(net, crate::ref_head(r))
        .into_iter()
        .map(|a| match readback(net, a) {
            Some(v) => format!("{:?}", v),
            None => "?".to_string(),
        })
        .collect();
    format!("{}({})", crate::ref_entry(r), args.join(","))
}

/// Speculatively unfold a partially static call in a clone of the state,
/// specialized to its fixpoint (its own calls unfolded, its branches'
/// arms instantiated). Accepted, replacing the state, when its control
/// was static: no call and no match remains of it (a branch on a runtime
/// value may, with its arms), and the net grew within budget.
#[allow(clippy::too_many_arguments)]
fn unfold_call(
    fx: &mut Fx,
    prog: &NetProg,
    eval_prog: &NetProg,
    idx: usize,
    fuel: u64,
    done: &mut u64,
    evaluated: &mut usize,
    memo: &mut Spec,
    depth: u32,
    frame: usize,
) -> bool {
    let (r, ret) = fx.net.residual[idx];
    let entry = crate::ref_entry(r) as usize;
    let live_before = agents(&fx.net);
    let data_before = data_nodes(&fx.net);
    // what is parked or residual before the unfold is not its doing
    let before = crate::residual::scan(&fx.net, &ROOTS, &fx.free, &fx.net.residual);
    let old_parked: HashSet<u32> = before.parked.iter().map(|(a, _)| *a).collect();
    let old_refs: HashSet<u64> = fx.net.residual.iter().filter(|(a, _)| a.tag() == Tag::Ref).map(|(a, _)| a.0).collect();
    let mut clone = fx.clone();
    clone.net.residual.swap_remove(idx);
    let args = crate::list_collect(&mut clone.net, crate::ref_head(r));
    crate::build::instantiate(&mut clone.net, prog, entry, args, ret);
    // a top-level attempt has its own rewrite budget and growth ceiling;
    // the unfolds nested in it (its body's own calls) spend from the same
    // budget, and the attempt as a whole is charged to the function's
    let outer_cap = memo.cap;
    let (ok, sub_done) = if depth == 0 {
        memo.cap = live_before + SPEC_UNFOLD_AGENTS;
        let mut sub_done = 0u64;
        let ok = settle(&mut clone, prog, eval_prog, SPEC_UNFOLD_FUEL, &mut sub_done, evaluated, memo, depth + 1, frame, &old_parked, &old_refs);
        // the copies count too: a chain clones the net per level
        memo.spent += sub_done + live_before as u64;
        (ok, sub_done)
    } else {
        let before = *done;
        let ok = settle(&mut clone, prog, eval_prog, fuel, done, evaluated, memo, depth + 1, frame, &old_parked, &old_refs);
        (ok, *done - before)
    };
    memo.cap = outer_cap;
    let reject = |why: &str| {
        if trace() {
            eprintln!("{}unfold of {}: {why}", "  ".repeat(depth as usize + 1), pattern_key(&fx.net, r));
        }
        false
    };
    if !ok {
        return reject("out of budget, or a call remains");
    }
    let live_after = agents(&clone.net);
    if live_after.saturating_sub(live_before) > SPEC_UNFOLD_AGENTS {
        return reject(&format!("too big (+{} agents)", live_after - live_before));
    }
    if clone.net.residual.iter().any(|(a, _)| a.tag() == Tag::Ref && !old_refs.contains(&a.0)) {
        return reject("a call remains");
    }
    // a partial unfold specializes control, it does not build data: a
    // constructor left over unknown fields would be allocation code
    // (a tuple is loop state and is fine)
    if data_nodes(&clone.net) > data_before {
        return reject("builds data");
    }
    let ix = crate::residual::scan(&clone.net, &ROOTS, &clone.free, &clone.net.residual);
    if ix.parked.iter().any(|(a, is_match)| *is_match && !old_parked.contains(a)) {
        return reject("a match remains");
    }
    if trace() {
        eprintln!("{}unfolded entry {entry} ({sub_done} rewrites, +{} agents)", "  ".repeat(depth as usize + 1), live_after.saturating_sub(live_before));
    }
    if depth == 0 {
        *done += sub_done;
    }
    *fx = clone;
    true
}

/// Live agent cells: both slots in use (an op, a branch, a constructor or
/// list link, a sharing node); a wire cell holds one port at most.
fn agents(net: &Net) -> usize {
    let mut freed = vec![false; net.cells.len()];
    for &i in &net.free {
        freed[i as usize] = true;
    }
    net.cells.iter().enumerate().filter(|(i, c)| !freed[*i] && c[0] != EMPTY.0 && c[1] != EMPTY.0).count()
}

/// Live constructor cells other than tuples, by the ports stored in wires
/// (a constructor is reachable only through a wire or another one's field).
fn data_nodes(net: &Net) -> usize {
    let mut freed = vec![false; net.cells.len()];
    for &i in &net.free {
        freed[i as usize] = true;
    }
    let mut n = 0;
    for (i, c) in net.cells.iter().enumerate() {
        if freed[i] {
            continue;
        }
        for p in [Port(c[0]), Port(c[1])] {
            if p.tag() == Tag::Con && p.con_tag() != crate::CTAG_TUPLE {
                n += 1;
            }
        }
    }
    n
}

/// Every argument of the call is a known value (no unfilled wire).
fn args_known(net: &Net, head: Port) -> bool {
    fn known(net: &Net, p: Port) -> bool {
        match p.tag() {
            Tag::Num | Tag::Flo => true,
            Tag::Con => crate::residual::con_fields(net, p).into_iter().all(|f| known(net, f)),
            Tag::Var => {
                let s = Port(net.cell(p.payload() as u32)[0]);
                s != EMPTY && s.tag() != Tag::Var && !matches!(s.tag(), Tag::Op | Tag::Swi | Tag::Mat | Tag::Dup | Tag::Ref) && known(net, s)
            }
            _ => false,
        }
    }
    crate::list_items(net, head).into_iter().all(|a| known(net, a))
}

/// Evaluate a fully static call in a scratch net (evaluation mode, every
/// call unfolds) under `SPEC_CALL_FUEL`; the value, re-materialized in the
/// main net, or `None` if it did not finish (or hit something opaque).
fn evaluate_call(net: &mut Net, eval_prog: &NetProg, r: Port) -> Option<Port> {
    let args: Vec<Val> = crate::list_items(net, crate::ref_head(r)).into_iter().map(|a| readback(net, a).expect("ICE: known argument without a value")).collect();
    let mut scratch = Net::new();
    let root = scratch.alloc(EMPTY, EMPTY);
    debug_assert_eq!(root, 0);
    let ps: Vec<Port> = args.iter().map(|v| alloc_val(&mut scratch, v)).collect();
    let head = crate::list_alloc(&mut scratch, &ps);
    scratch.redexes.push((crate::ref_port(head, crate::ref_entry(r)), crate::root_port()));
    let mut done = 0u64;
    let mut i = 0usize;
    while done < SPEC_CALL_FUEL && i < scratch.redexes.len() {
        let (a, b) = scratch.redexes[i];
        i += 1;
        done += process(&mut scratch, eval_prog, a, b);
    }
    if i < scratch.redexes.len() || !scratch.residual.is_empty() {
        return None;
    }
    // a large value is not worth embedding in the program
    if scratch.cells.len() - scratch.free.len() > SPEC_VALUE_CELLS {
        return None;
    }
    let v = readback(&scratch, crate::root_port())?;
    // the call's argument list is consumed: free it
    let _ = crate::list_collect(net, crate::ref_head(r));
    Some(alloc_val(net, &v))
}

/// Materialize a value as ports/cells.
fn alloc_val(net: &mut Net, v: &Val) -> Port {
    match v {
        Val::I(n) => Port::num(*n),
        Val::F(f) => crate::rules::flo_alloc(net, *f),
        Val::C(cid, fields) => {
            let ps: Vec<Port> = fields.iter().map(|f| alloc_val(net, f)).collect();
            let tag = if *cid == mithril_front::core::UNREACHABLE_CTOR { CTAG_UNREACHABLE } else { *cid as u16 };
            crate::con_alloc(net, tag, &ps)
        }
        Val::T(items) => {
            let ps: Vec<Port> = items.iter().map(|f| alloc_val(net, f)).collect();
            crate::con_alloc(net, CTAG_TUPLE, &ps)
        }
        Val::A(_) | Val::L(..) => panic!("ICE: an array or closure value at compile time"),
    }
}

/// Every Dup agent reachable now that has no frame yet belongs to `frame`.
fn assign_dups(net: &Net, roots: &[Port], free: &[(Port, u32)], dup_frame: &mut HashMap<u32, usize>, frame: usize) {
    let ix = crate::residual::scan(net, roots, free, &net.residual);
    for p in ix.producers() {
        if let crate::residual::Producer::Dup(d) = p {
            dup_frame.entry(*d).or_insert(frame);
        }
    }
}
fn max_var(e: &Core) -> u32 {
    match e {
        Core::Var(v) => *v,
        Core::Num(_) | Core::Flo(_) => 0,
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => max_var(a).max(max_var(b)),
        Core::Let(v, a, b) => (*v).max(max_var(a)).max(max_var(b)),
        Core::If(a, b, c) => max_var(a).max(max_var(b)).max(max_var(c)),
        Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Prim(_, xs) => xs.iter().map(max_var).max().unwrap_or(0),
        Core::Reuse(v, _, xs) => (*v).max(xs.iter().map(max_var).max().unwrap_or(0)),
        Core::Match(s, arms) => {
            let mut m = max_var(s);
            for (_, bs, b) in arms {
                m = m.max(bs.iter().copied().max().unwrap_or(0)).max(max_var(b));
            }
            m
        }
        Core::Proj(a, _) => max_var(a),
        Core::Lam(x, b) => (*x).max(max_var(b)),
        Core::App(f, a) => max_var(f).max(max_var(a)),
    }
}
