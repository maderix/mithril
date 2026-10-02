//! Share call machinery when continuation captures have the same layout.
use super::*;

pub(super) fn share(m: &mut Machine<'_>) {
    let captures = m.captures();
    let calls: Vec<_> = m
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(i, b)| {
            let End::Call(f, args, outs, next) = &b.end else {
                return None;
            };
            let saved = captures[&i].clone();
            Some((i, f.clone(), args.clone(), outs.clone(), *next, saved))
        })
        .collect();
    if calls.len() < 2 {
        return;
    }
    let (_, f, args, outs, _, saved) = &calls[0];
    let fields = storage::Frame::layout(m, saved);
    // One shared call eliminates all return-PC words. The selector is one
    // word; every capture retains its original width and representation.
    if calls.iter().any(|(_, g, a, o, _, s)| {
        g != f || a.len() != args.len() || o.len() != outs.len() || storage::Frame::layout(m, s) != fields
    }) {
        return;
    }
    let callee = f.clone();
    let captures: Vec<_> = fields
        .into_iter()
        .map(|(ty, size)| {
            let n = m.local(ty);
            if size == 1 {
                m.narrow.insert(n.clone());
            }
            n
        })
        .collect();
    let selector = m.local(Ty::U32);
    m.narrow.insert(selector.clone());
    let arguments: Vec<_> = (0..args.len())
        .map(|j| {
            if calls.iter().all(|c| c.2[j] == args[j]) {
                args[j].clone()
            } else {
                v(m.local(Ty::I64))
            }
        })
        .collect();
    let results: Vec<_> = outs.iter().map(|_| m.local(Ty::I64)).collect();
    let call = m.block();
    let dispatch = m.block();
    let mut arms = Vec::new();
    for (key, (at, _, args, outs, next, saved)) in calls.into_iter().enumerate() {
        for (target, value) in arguments.iter().zip(args) {
            if let E::V(n) = target {
                if *target != value {
                    m.blocks[at].body.push(set(n, value));
                }
            }
        }
        m.blocks[at]
            .body
            .extend(captures.iter().zip(&saved).map(|(to, from)| set(to, v(from))));
        m.blocks[at].body.push(set(&selector, u32_(key as u64)));
        m.blocks[at].end = End::Jump(call);
        let restore = m.block();
        m.blocks[restore]
            .body
            .extend(saved.iter().zip(&captures).map(|(to, from)| set(to, v(from))));
        m.blocks[restore]
            .body
            .extend(outs.iter().zip(&results).map(|(to, from)| set(to, v(from))));
        m.blocks[restore].end = End::Jump(next);
        arms.push((key as u64, restore));
    }
    m.blocks[call].end = End::Call(callee, arguments, results, dispatch);
    m.blocks[dispatch].end = End::Switch(v(selector), arms, None);
}

#[cfg(test)]
#[path = "../tests/support/native_join.rs"]
mod tests;
