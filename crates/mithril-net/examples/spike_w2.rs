//! Derisk probe for Phase B (closures shared by DUP at runtime): spike 5's
//! W2 shape on the real rule table and reducer.
//!
//!   g = λx. x + heavy(k)      heavy = fib-shaped, x-independent
//!   Σ_{i<N} g(i)
//!
//! A strict evaluator does N·W work (W = heavy's work). The net builds the
//! closure once and shares it through a chain of DUPs: the claim is
//! rewrites = W + c·N with a small c. Usage: spike_w2 [k] [N...]

use mithril_core::net::Net;
use mithril_core::port::{Port, Tag};
use mithril_net::EMPTY;
use mithril_front::ast::BinOp;
use mithril_front::{desugar, parse};
use mithril_net::{dup_port, link, list_alloc, op_port, opcode_bin, readback, reduce, ref_port, wire};

fn fib(k: i64) -> i64 {
    if k < 2 { k } else { fib(k - 1) + fib(k - 2) }
}

fn build(k: i64, n: usize, late: bool) -> Net {
    let mut net = Net::new();
    let root = net.alloc(EMPTY, EMPTY);
    assert_eq!(root, 0);
    // heavy(k) -> hv, fired once
    let hv = wire(&mut net);
    let head = list_alloc(&mut net, &[Port::num(k)]);
    let heavy = (ref_port(head, 0), hv);
    if !late {
        net.redexes.push(heavy);
    }
    // g = λp. p + hv : Lam cell [p, b]; the Op waits on p, its other operand is hv
    let p = wire(&mut net);
    let b = wire(&mut net);
    let add = net.alloc(hv, b);
    link(&mut net, op_port(add, opcode_bin(BinOp::Add)), p);
    let l = net.alloc(p, b);
    let lam = Port::new(Tag::Lam, l as u64);
    // N copies through a chain of DUPs
    let mut cur = lam;
    let mut copies = Vec::with_capacity(n);
    for i in 0..n {
        if i + 1 < n {
            let (w1, w2) = (wire(&mut net), wire(&mut net));
            let d = net.alloc(w1, w2);
            link(&mut net, dup_port(d), cur);
            copies.push(w1);
            cur = w2;
        } else {
            copies.push(cur);
        }
    }
    // g(i) for each copy, summed into the root
    let mut acc: Option<Port> = None;
    for (i, f) in copies.into_iter().enumerate() {
        let t = wire(&mut net);
        let app = net.alloc(Port::num(i as i64), t);
        link(&mut net, Port::new(Tag::App, app as u64), f);
        acc = Some(match acc {
            None => t,
            Some(s) => {
                let r = wire(&mut net);
                let a = net.alloc(s, r);
                link(&mut net, op_port(a, opcode_bin(BinOp::Add)), t);
                r
            }
        });
    }
    link(&mut net, acc.unwrap(), Port::new(Tag::Var, 0));
    if late {
        // the copies and applications are queued before heavy runs
        net.redexes.push(heavy);
    }
    net
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let k: i64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(24);
    let ns: Vec<usize> = if args.len() > 2 { args[2..].iter().map(|s| s.parse().unwrap()).collect() } else { vec![1, 16, 256, 4096] };
    let src = "def heavy(k):\n    if k < 2:\n        return k\n    return heavy(k - 1) + heavy(k - 2)\n\ndef main():\n    return 0\n";
    let m = desugar(&parse(src).unwrap()).unwrap();
    let mut w = 0u64;
    println!("{:>6} {:>12} {:>12} {:>10} {:>9}  ok", "N", "rewrites", "strict N*W", "per-app", "seconds");
    for late in [false, true] {
    println!("-- heavy(k) {} the copies", if late { "after" } else { "before" });
    for &n in &ns {
        let mut net = build(k, n, late);
        let t = std::time::Instant::now();
        let done = reduce(&mut net, &m, u64::MAX);
        let secs = t.elapsed().as_secs_f64();
        assert!(net.redexes.is_empty());
        let got = readback(&net, mithril_net::root_port());
        let want: i64 = (0..n as i64).map(|i| i + fib(k)).sum();
        let ok = got == Some(mithril_front::Val::I(want));
        if n == 1 {
            w = done;
        }
        let per_app = if n > 1 { (done.saturating_sub(w)) as f64 / (n - 1) as f64 } else { 0.0 };
        println!("{:>6} {:>12} {:>12} {:>10.1} {:>9.3}  {}", n, done, w * n as u64, per_app, secs, if ok { "yes".to_string() } else { format!("NO {:?}", got) });
    }
    }
}
