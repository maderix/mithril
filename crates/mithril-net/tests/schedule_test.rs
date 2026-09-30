//! Schedule conformance: random first-order programs, built as nets and
//! reduced by the rule table under random redex orders, must all reach the
//! oracle's value (confluence). Each program is reduced under 5 sampled
//! orders. The rewrite count is also compared (the diamond property); it
//! does not hold for the extended table (see the ignored test). `MITHRIL_CONFORMANCE_N` sets the
//! number of programs (default 10,000).

use mithril_front::core::{CoreModule, Val};
use mithril_front::{desugar, eval_core, parse};
use mithril_net::rules::process;
use mithril_net::{build, readback, root_port, NetProg};

fn xorshift(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

/// A random program: helpers h0..h2 (each may call the earlier ones), the
/// fixed helpers ap, pair, build and total, and a main that shares an int,
/// a closure (applied twice) and a list (consumed twice).
/// No division, modulo or shifts; the only recursion (build, total) is
/// bounded by a literal at most 6; main returns a tuple of ints.
struct Gen {
    s: u64,
}

impl Gen {
    fn pick(&mut self, n: u64) -> u64 {
        xorshift(&mut self.s) % n
    }

    fn expr(&mut self, vars: &[&str], helpers: usize, d: u32) -> String {
        let k = if d == 0 { self.pick(2) } else { self.pick(8) };
        match k {
            0 => format!("{}", self.pick(20)),
            1 if !vars.is_empty() => vars[self.pick(vars.len() as u64) as usize].to_string(),
            1 => format!("{}", self.pick(20)),
            2 => {
                let op = ["+", "-", "*", "&", "^"][self.pick(5) as usize];
                format!("({} {} {})", self.expr(vars, helpers, d - 1), op, self.expr(vars, helpers, d - 1))
            }
            3 => format!(
                "({} if {} < {} else {})",
                self.expr(vars, helpers, d - 1),
                self.expr(vars, helpers, d - 1),
                self.expr(vars, helpers, d - 1),
                self.expr(vars, helpers, d - 1)
            ),
            4 if helpers > 0 => format!("h{}({}, {})", self.pick(helpers as u64), self.expr(vars, helpers, d - 1), self.expr(vars, helpers, d - 1)),
            4 => format!("{}", self.pick(20)),
            5 => format!("ap(lambda x: x * {} + {}, {})", self.expr(vars, helpers, d - 1), self.pick(9), self.expr(vars, helpers, d - 1)),
            6 => format!("total(build({}, {}))", self.pick(7), self.expr(vars, helpers, d - 1)),
            _ => format!("pair({}, {})[{}]", self.expr(vars, helpers, d - 1), self.expr(vars, helpers, d - 1), self.pick(2)),
        }
    }

    fn program(&mut self) -> String {
        let mut s = String::from(
            "@data\nclass L:\n    Nil: ()\n    Cons: (h, t)\n\n\n\
             def ap(f, v):\n    return f(v)\n\n\n\
             def pair(a, b):\n    return (a, b)\n\n\n\
             def build(n, s):\n    if n == 0:\n        return Nil()\n    return Cons(s + n, build(n - 1, s))\n\n\n\
             def total(l):\n    match l:\n        case Nil():\n            return 0\n        case Cons(h, t):\n            return h + total(t)\n\n\n",
        );
        for i in 0..3 {
            let x = self.expr(&["a", "b"], i, 3);
            let r = self.expr(&["a", "b", "x"], i, 3);
            s.push_str(&format!("def h{i}(a, b):\n    x = {x}\n    return {r}\n\n\n"));
        }
        let y = self.expr(&[], 3, 3);
        let k = self.expr(&["y"], 3, 2);
        let n = self.pick(7);
        let r1 = self.expr(&["y"], 3, 3);
        let r2 = self.expr(&["y"], 3, 3);
        // a closure shared and applied twice, a list shared and consumed twice
        s.push_str(&format!(
            "def main():\n    y = {y}\n    f = lambda x: x * {k} + y\n    l = build({n}, y)\n    return ({r1}, {r2}, f({r1}) + f(y), total(l) + total(l))\n"
        ));
        s
    }
}

fn program(p: u64) -> String {
    Gen { s: p.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1 }.program()
}

fn module(src: &str) -> CoreModule {
    let m = parse(src).unwrap_or_else(|d| panic!("parse error line {}: {}\n{src}", d.line, d.msg));
    desugar(&m).unwrap_or_else(|d| panic!("desugar error line {}: {}\n{src}", d.line, d.msg))
}

fn count() -> u64 {
    std::env::var("MITHRIL_CONFORMANCE_N").ok().and_then(|v| v.parse().ok()).unwrap_or(10_000)
}

/// Build `m`'s net and fire its redexes in a random order (`seed`) until
/// none is left. Returns the value read back and the rewrites counted.
fn reduce_shuffled(m: &CoreModule, seed: u64) -> (Option<Val>, u64) {
    let prog = NetProg::new(m);
    let mut net = build(m);
    let mut rng = seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1;
    let (mut steps, mut fired) = (0u64, 0u64);
    while !net.redexes.is_empty() {
        let i = (xorshift(&mut rng) % net.redexes.len() as u64) as usize;
        let (a, b) = net.redexes.swap_remove(i);
        steps += process(&mut net, &prog, a, b);
        fired += 1;
        assert!(fired < 20_000_000, "no quiescence under seed {seed}");
    }
    assert!(net.residual.is_empty(), "evaluation left residual redexes");
    (readback(&net, root_port()), steps)
}

#[test]
fn generated_programs_terminate_and_return_ints() {
    for p in 1..=50u64 {
        let src = program(p);
        for bad in [" / ", " // ", " % ", "<<", ">>"] {
            assert!(!src.contains(bad), "{bad} in:\n{src}");
        }
        let m = module(&src);
        match eval_core(&m, m.main, &[]) {
            Val::T(fs) => assert!(fs.iter().all(|v| matches!(v, Val::I(_))), "not a tuple of ints:\n{src}"),
            v => panic!("main returned {v:?}, not a tuple:\n{src}"),
        }
    }
}

#[test]
fn every_schedule_reaches_the_oracle() {
    for p in 1..=count() {
        let src = program(p);
        let m = module(&src);
        let want = eval_core(&m, m.main, &[]);
        for seed in 0..5 {
            let (got, _) = reduce_shuffled(&m, seed);
            assert_eq!(got, Some(want.clone()), "program {p}, schedule {seed}:\n{src}");
        }
    }
}

#[test]
#[ignore = "most programs take order-dependent rewrite counts (a spread of about 1% per program): OP's half step fires only when an operand arrives first; values never differ. Lean P2 decides the extended table's step claim."]
fn every_schedule_takes_the_same_number_of_rewrites() {
    let mut differ = Vec::new();
    for p in 1..=count() {
        let src = program(p);
        let m = module(&src);
        let steps: Vec<u64> = (0..5).map(|seed| reduce_shuffled(&m, seed).1).collect();
        if steps.iter().any(|&s| s != steps[0]) {
            differ.push((p, steps, src));
        }
    }
    assert!(
        differ.is_empty(),
        "{} programs take schedule-dependent rewrite counts; first: program {} {:?}\n{}",
        differ.len(),
        differ[0].0,
        differ[0].1,
        differ[0].2
    );
}
