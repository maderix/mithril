//! Oracle check of specialization on a whole program: every function's
//! specialized body is compared against its original on the arguments the
//! original program actually calls it with. Usage: spec_oracle FILE.py
//! (the program must be small enough for the interpreter oracle).

use mithril_front::core::eval_core;
use mithril_front::{desugar, parse};

fn main() {
    let path = std::env::args().nth(1).expect("usage: spec_oracle FILE.py");
    let src = std::fs::read_to_string(&path).unwrap();
    let mut m = parse(&src).unwrap_or_else(|d| panic!("parse: {} line {}", d.msg, d.line));
    let _ = mithril_reassoc::analyze(&mut m);
    let m = desugar(&m).unwrap_or_else(|d| panic!("desugar: {} line {}", d.msg, d.line));
    let (s, _) = mithril_net::specialize(&m, 1 << 24);
    let handle = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let want = eval_core(&m, m.main, &[]);
            let got = std::panic::catch_unwind(|| eval_core(&s, s.main, &[])).ok();
            println!("original {:?}\nspecialized {:?}", want, got);
            if got == Some(want.clone()) {
                return;
            }
            // find a culprit: one function at a time
            let mut bad = Vec::new();
            for i in 0..m.fns.len() {
                let mut mixed = m.clone();
                mixed.fns[i] = s.fns[i].clone();
                let v = std::panic::catch_unwind(|| eval_core(&mixed, mixed.main, &[]));
                match v {
                    Ok(v) if v == want => {}
                    Ok(v) => {
                        bad.push(i);
                        println!("fn {} ({}) alone changes the value to {:?}", i, m.fns[i].name, v);
                    }
                    Err(_) => {
                        bad.push(i);
                        println!("fn {} ({}) alone panics", i, m.fns[i].name);
                    }
                }
            }
            for i in &bad {
                println!("--- {} original:\n{:?}\n--- specialized:\n{:?}", m.fns[*i].name, m.fns[*i].body, s.fns[*i].body);
            }
            std::process::exit(1);
        })
        .unwrap();
    handle.join().unwrap();
}
