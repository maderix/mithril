//! Share identical paths through a native continuation, preserving snapshots.
use super::*;

fn cost(body: &[S]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    walk_exprs(body, &mut |e| {
        let key = match e {
            E::Call { f, .. } => format!("call:{f}"),
            E::Bin(op, ..) => format!("bin:{op:?}"),
            E::Neg(_) => "neg".into(),
            E::Not(_) => "not".into(),
            _ => return,
        };
        *counts.entry(key).or_default() += 1;
    });
    counts
}
/// Resolve copies along a straight path. Writes to live destinations use
/// their pre-path values; cyclic copies retain the original path.
fn canonical(body: &[S], live: &BTreeSet<String>) -> Option<Vec<S>> {
    let mut env = Env::new();
    let mut order = Vec::new();
    for s in body {
        let S::Set(n, e) = s else { return None };
        if !operations::Policy::Copy.accepts(e) {
            return None;
        }
        env.insert(n.clone(), e.substitute(&env, false).unwrap());
        order.retain(|old| old != n);
        order.push(n.clone());
    }
    let mut values: Env = env
        .into_iter()
        .filter(|(n, e)| live.contains(n) && *e != v(n))
        .collect();
    let mut out = Vec::new();
    while !values.is_empty() {
        let n = order
            .iter()
            .find(|n| {
                values.contains_key(*n)
                    && values
                        .iter()
                        .all(|(other, e)| other == *n || !e.reads().contains(*n))
            })?
            .clone();
        out.push(set(&n, values.remove(&n).unwrap()));
    }
    let before = cost(body);
    cost(&out)
        .iter()
        .all(|(k, n)| *n <= before.get(k).copied().unwrap_or(0))
        .then_some(out)
}
pub(super) fn copies(m: &mut Machine<'_>) {
    let live = m.live();
    let mut changes = Vec::new();
    for (at, b) in m.blocks.iter().enumerate() {
        let mut path = b.body.clone();
        let mut end = &b.end;
        let mut seen = BTreeSet::from([at]);
        while let End::Jump(next) = end {
            if !seen.insert(*next) || !matches!(m.blocks[*next].end, End::Jump(_)) {
                break;
            }
            path.extend(m.blocks[*next].body.clone());
            end = &m.blocks[*next].end;
        }
        if let End::Jump(next) = end {
            // The shared work counter is deliberately outside frame liveness.
            let mut needed = live[*next].clone();
            needed.insert("native_work".into());
            if let Some(body) = canonical(&path, &needed) {
                changes.push((at, body, end.clone()));
            }
        }
    }
    for (at, body, end) in changes {
        m.blocks[at] = Block { body, end };
    }
}

/// Case-local selector updates can be expressed once when they differ from
/// their incoming case keys by the same XOR. Every case proves its own input.
pub(super) fn selectors(m: &mut Machine<'_>) {
    let incoming = m.incoming();
    let private =
        |to: usize| incoming[to] == 1 && !m.entries.values().any(|n| *n == to) && !m.loops.contains(&to);
    let mut updates = Vec::new();
    for b in &m.blocks {
        let End::Switch(E::V(selector), arms, _) = &b.end else {
            continue;
        };
        for (base_key, target) in arms {
            if !private(*target) {
                continue;
            }
            let base = &m.blocks[*target].body;
            for (at, s) in base.iter().enumerate() {
                let Some((name, value, ty)) = constant(s) else {
                    continue;
                };
                if name != selector
                    || base[..at]
                        .iter()
                        .any(|s| !matches!(s, S::Set(n, e) if n != selector && operations::Policy::Copy.accepts(e)))
                {
                    continue;
                }
                let group: Vec<_> = arms.iter().filter(|(_, to)| {
                    let path = &m.blocks[*to].body;
                    private(*to) && matches!((&m.blocks[*target].end, &m.blocks[*to].end), (End::Jump(a), End::Jump(b)) if a==b)
                        && path.len()==base.len() && path.iter().zip(base).enumerate().all(|(i,(a,b))| {
                            if i==at { constant(a).map(|(n,_,t)| (n,t))==constant(b).map(|(n,_,t)| (n,t)) } else { a==b }
                        })
                }).collect();
                let delta = value as u64 ^ *base_key;
                if group.len() < 2
                    || group
                        .iter()
                        .any(|(key, to)| constant(&m.blocks[*to].body[at]).unwrap().1 as u64 ^ *key != delta)
                {
                    continue;
                }
                for (_, to) in group {
                    updates.push((
                        *to,
                        at,
                        set(
                            selector,
                            cast(bin(Bop::Xor, cast(v(selector), Ty::I64), i64_(delta as i64)), ty),
                        ),
                    ));
                }
            }
        }
    }
    for (to, at, s) in updates {
        m.blocks[to].body[at] = s;
    }
}

fn constant(s: &S) -> Option<(&str, i64, Ty)> {
    if let S::Set(n, E::Int(value, ty)) = s {
        Some((n, *value, *ty))
    } else {
        None
    }
}
/// Factor paths differing only in a constant selector. The selector switch
/// retains every original key and default, then executes one common body.
pub(super) fn factor(body: &mut Vec<S>) {
    for s in body.iter_mut() {
        for b in s.parts_mut().1 {
            factor(b);
        }
    }
    for s in body.iter_mut() {
        let S::Switch(e, arms, default) = s else { continue };
        if !operations::Policy::Copy.accepts(e) {
            continue;
        }
        let Some(base) = default.as_ref() else { continue };
        let mut best = None;
        for (at, statement) in base.iter().enumerate() {
            let Some((name, _, _)) = constant(statement) else {
                continue;
            };
            // Moving the selector before the common prefix cannot affect a
            // read of its previous value, or cross an effect/control barrier.
            if base[..at]
                .iter()
                .any(|s| !matches!(s, S::Set(n, x) if n != name && operations::Policy::Copy.accepts(x) && !x.reads().contains(name)))
            {
                continue;
            }
            let equal = |b: &[S]| {
                b.len() == base.len()
                    && b.iter().zip(base).enumerate().all(|(i, (a, b))| {
                        if i == at {
                            constant(a).map(|(n, _, t)| (n, t)) == constant(b).map(|(n, _, t)| (n, t))
                        } else {
                            a == b
                        }
                    })
            };
            let keys: Vec<_> = arms.iter().filter(|(_, b)| equal(b)).map(|(k, _)| *k).collect();
            if !keys.is_empty() {
                best = Some((at, keys));
                break;
            }
        }
        let Some((at, keys)) = best else { continue };
        let mut common = base.clone();
        let selector_default = common.remove(at);
        let mut selections = Vec::new();
        arms.retain(|(k, b)| {
            if keys.contains(k) {
                selections.push((*k, vec![b[at].clone()]));
                false
            } else {
                true
            }
        });
        let mut shared = vec![S::Switch(e.clone(), selections, Some(vec![selector_default]))];
        shared.extend(common);
        *default = Some(shared);
    }
}

#[cfg(test)]
#[path = "../tests/support/native_paths.rs"]
mod tests;
