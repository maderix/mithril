//! Run the same input net under selectable reduction orders, without changing rules.

use mithril_core::net::Net;
use mithril_core::port::Port;
use mithril_front::core::{CoreModule, Val};
use mithril_front::{desugar, eval_core, parse};
use mithril_net::rules::process;
use mithril_net::{build, readback, root_port, NetProg};
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

struct Measurement {
    value: Val,
    rewrites: u64,
    peak_cells: usize,
    final_cells: usize,
    peak_pending: usize,
    pops: u64,
    trace: Vec<[u64; 4]>,
}

#[derive(Clone)]
struct Execution {
    net: Net,
    ready: VecDeque<(Port, Port)>,
    rng: u64,
    rewrites: u64,
    pops: u64,
    peak_pending: usize,
    trace: Vec<[u64; 4]>,
}

impl Execution {
    fn new(module: &CoreModule, seed: u64) -> Self {
        let mut net = build(module);
        let ready: VecDeque<_> = net.redexes.drain(..).collect();
        let mut run = Self {
            net,
            peak_pending: ready.len(),
            ready,
            rng: seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1,
            rewrites: 0,
            pops: 0,
            trace: Vec::new(),
        };
        run.sample();
        run
    }

    fn sample(&mut self) {
        let point = [
            self.pops,
            self.rewrites,
            (self.net.cells.len() - self.net.free.len()) as u64,
            self.ready.len() as u64,
        ];
        if self.trace.last() != Some(&point) {
            self.trace.push(point);
        }
    }

    fn advance(&mut self, prog: &NetProg, policy: &str, budget: u64) -> Result<bool, String> {
        if !["lifo", "fifo", "random", "alternate"].contains(&policy) {
            return Err(format!("unknown policy: {policy}"));
        }
        for _ in 0..budget {
            if self.ready.is_empty() {
                break;
            }
            let order = if policy == "alternate" {
                if (self.pops / 1024) % 2 == 0 {
                    "lifo"
                } else {
                    "fifo"
                }
            } else {
                policy
            };
            let pair = match order {
                "lifo" => self.ready.pop_back(),
                "fifo" => self.ready.pop_front(),
                _ => {
                    self.rng ^= self.rng << 13;
                    self.rng ^= self.rng >> 7;
                    self.rng ^= self.rng << 17;
                    self.ready
                        .swap_remove_back((self.rng % self.ready.len() as u64) as usize)
                }
            }
            .unwrap();
            self.rewrites += process(&mut self.net, prog, pair.0, pair.1);
            self.pops += 1;
            self.ready.extend(self.net.redexes.drain(..));
            self.peak_pending = self.peak_pending.max(self.ready.len());
            if self.pops % 4096 == 0 {
                self.sample();
            }
        }
        self.sample();
        Ok(self.ready.is_empty())
    }

    fn fingerprint(&self) -> u64 {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.net.cells.hash(&mut hash);
        self.net.free.hash(&mut hash);
        self.net.labels.hash(&mut hash);
        self.ready.len().hash(&mut hash);
        for (a, b) in &self.ready {
            (a.0, b.0).hash(&mut hash);
        }
        self.net.residual.len().hash(&mut hash);
        for (a, b) in &self.net.residual {
            (a.0, b.0).hash(&mut hash);
        }
        self.rng.hash(&mut hash);
        hash.finish()
    }

    fn finish(self) -> Result<Measurement, String> {
        if !self.ready.is_empty() {
            return Err("unfinished work remains".into());
        }
        if !self.net.residual.is_empty() {
            return Err("opaque or stuck operations remain residual".into());
        }
        let value = readback(&self.net, root_port()).ok_or("no complete result")?;
        // Arena growth only occurs with no free slots: length is its exact
        // peak occupied count, including transient allocations inside a rule.
        Ok(Measurement {
            value,
            rewrites: self.rewrites,
            peak_cells: self.net.cells.len(),
            final_cells: self.net.cells.len() - self.net.free.len(),
            peak_pending: self.peak_pending,
            pops: self.pops,
            trace: self.trace,
        })
    }
}

fn measure(
    module: &CoreModule,
    policy: &str,
    seed: u64,
    limit: u64,
) -> Result<Measurement, String> {
    let prog = NetProg::new(module);
    let mut run = Execution::new(module, seed);
    if !run.advance(&prog, policy, limit)? {
        return Err(format!("did not finish within {limit} worklist pops"));
    }
    run.finish()
}

fn json_value(value: &Val) -> String {
    match value {
        Val::I(n) => n.to_string(),
        Val::T(items) => format!(
            "[{}]",
            items.iter().map(json_value).collect::<Vec<_>>().join(",")
        ),
        _ => panic!("probe expects only integer/tuple results"),
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(
        (2..=4).contains(&args.len()),
        "usage: compiler_planning SOURCE [RANDOM_SEEDS] [--demo]"
    );
    let seeds: u64 = args
        .get(2)
        .map(|s| s.parse().expect("seed count"))
        .unwrap_or(8);
    assert!(seeds <= 100, "at most 100 sampled random orders");
    let source = std::fs::read_to_string(&args[1]).expect("read source");
    let module = desugar(&parse(&source).expect("parse")).expect("desugar");
    let expected = eval_core(&module, module.main, &[]);
    let demo = args.get(3).map(String::as_str) == Some("--demo");
    if args.len() == 4 {
        assert!(demo, "unknown option");
    }
    for (policy, seed) in [("lifo", 0), ("fifo", 0)]
        .into_iter()
        .chain((0..seeds).map(|s| ("random", s)))
    {
        let run = measure(&module, policy, seed, 20_000_000)
            .unwrap_or_else(|e| panic!("{policy}/{seed}: {e}"));
        assert_eq!(
            run.value, expected,
            "{policy}/{seed}: result differs from Core oracle"
        );
        emit(&run, policy, seed, 0, "", 0, demo);
    }
    if demo {
        let run = measure(&module, "alternate", 0, 20_000_000).unwrap();
        assert_eq!(run.value, expected);
        emit(&run, "alternate", 0, 0, "", 0, true);
        let prog = NetProg::new(&module);
        for prefix_policy in ["lifo", "fifo"] {
            for checkpoint in [17, 4096] {
                let mut prefix = Execution::new(&module, 0);
                let complete = prefix.advance(&prog, prefix_policy, checkpoint).unwrap();
                assert!(!complete, "checkpoint must have pending work");
                let fingerprint = prefix.fingerprint();
                for policy in ["lifo", "fifo", "random"] {
                    let mut resumed = prefix.clone();
                    assert!(resumed
                        .advance(&prog, policy, 20_000_000 - checkpoint)
                        .unwrap());
                    let run = resumed.finish().unwrap();
                    assert_eq!(run.value, expected, "checkpoint continuation differs");
                    emit(
                        &run,
                        policy,
                        0,
                        checkpoint,
                        prefix_policy,
                        fingerprint,
                        true,
                    );
                    assert_eq!(
                        prefix.fingerprint(),
                        fingerprint,
                        "continuation changed saved prefix"
                    );
                }
            }
        }
    }
}

fn emit(
    run: &Measurement,
    policy: &str,
    seed: u64,
    checkpoint: u64,
    prefix: &str,
    fingerprint: u64,
    with_trace: bool,
) {
    let trace = if with_trace {
        format!(
            "[{}]",
            run.trace
                .iter()
                .map(|p| format!("[{},{},{},{}]", p[0], p[1], p[2], p[3]))
                .collect::<Vec<_>>()
                .join(",")
        )
    } else {
        "[]".into()
    };
    println!("{{\"policy\":\"{policy}\",\"seed\":{seed},\"checkpoint\":{checkpoint},\"prefix_policy\":\"{prefix}\",\"checkpoint_hash\":\"{fingerprint:016x}\",\"rewrites\":{},\"pops\":{},\"peak_cells\":{},\"final_cells\":{},\"peak_pending\":{},\"trace\":{trace},\"value\":{},\"oracle_equal\":true}}",
        run.rewrites, run.pops, run.peak_cells, run.final_cells, run.peak_pending, json_value(&run.value));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module() -> CoreModule {
        desugar(&parse("def f(x):\n    return x * x + 1\n\ndef main():\n    x = f(7)\n    return (x + x, f(x))\n").unwrap()).unwrap()
    }

    #[test]
    fn orders_preserve_shared_computation_results() {
        let m = module();
        let expected = Val::T(vec![Val::I(100), Val::I(2501)].into());
        assert_eq!(eval_core(&m, m.main, &[]), expected);
        for policy in ["lifo", "fifo", "random"] {
            for seed in 0..4 {
                let run = measure(&m, policy, seed, 100_000).unwrap();
                assert_eq!(run.value, expected);
                assert!(run.rewrites > 0);
                assert!(run.peak_cells > 0);
                assert!(run.peak_cells >= run.final_cells);
            }
        }
    }

    #[test]
    fn limits_and_unknown_policies_fail_explicitly() {
        assert!(measure(&module(), "lifo", 0, 0).is_err());
        assert!(measure(&module(), "unknown", 0, 100).is_err());
    }

    #[test]
    fn opaque_arrays_cannot_be_reported_as_complete() {
        let m = desugar(&parse("def main():\n    return array_get(array_new(1, 7), 0)\n").unwrap())
            .unwrap();
        assert!(measure(&m, "lifo", 0, 100_000).is_err());
    }

    #[test]
    fn random_order_is_reproducible() {
        let m = module();
        let a = measure(&m, "random", 19, 100_000).unwrap();
        let b = measure(&m, "random", 19, 100_000).unwrap();
        assert_eq!(a.value, b.value);
        assert_eq!(
            (a.rewrites, a.pops, a.peak_cells, a.peak_pending),
            (b.rewrites, b.pops, b.peak_cells, b.peak_pending)
        );
    }

    #[test]
    fn checkpoint_forks_resume_without_replaying_the_prefix() {
        let m = module();
        let prog = NetProg::new(&m);
        let mut prefix = Execution::new(&m, 3);
        assert!(!prefix.advance(&prog, "lifo", 3).unwrap());
        let before = prefix.fingerprint();
        let expected = eval_core(&m, m.main, &[]);
        for policy in ["lifo", "fifo", "random", "alternate"] {
            let mut resumed = prefix.clone();
            assert_eq!(resumed.pops, 3);
            assert!(resumed.advance(&prog, policy, 100_000).unwrap());
            assert_eq!(resumed.finish().unwrap().value, expected);
        }
        assert_eq!(prefix.fingerprint(), before);
        assert_eq!(prefix.pops, 3);
    }

    #[test]
    fn lifted_loop_joins_have_the_hand_checked_result_in_every_order() {
        let source = "def main():\n    total = 0\n    for i in range(3):\n        if i % 2 == 0:\n            weight = 2\n        else:\n            weight = 3\n        if weight > 0:\n            total = total + weight\n    return total\n";
        let m = desugar(&parse(source).unwrap()).unwrap();
        for policy in ["lifo", "fifo", "random", "alternate"] {
            for seed in 0..4 {
                assert_eq!(measure(&m, policy, seed, 100_000).unwrap().value, Val::I(7));
            }
        }
    }
}
