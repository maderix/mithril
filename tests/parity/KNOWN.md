# Known divergences of the reference

The reference is commit 87e015b3e24e04cb0ccc1ee77b3f7cc7e5fe2579 plus the `mithril oracle` hook
(`build_ref.py`). These are the lanes of the reference that disagree with
each other on the parity corpus: today's bugs. `check.py --divergences`
prints them from `reference.json`.

The correct answer is the oracle's (`mithril oracle`, `eval_core`), except
where the net's lazy semantics decides (an unused binding is never
evaluated) or the oracle cannot run the program (`known.json` then states the
answer and why). Every answer below was also checked by hand or against the
program's documented value.

A candidate that prints the correct answer on one of these lanes is FIXED;
one that keeps the reference's wrong answer is EQUAL (parity, not
correctness, is the acceptance criterion); anything else is REGRESSED.

Lanes: `oracle`; `run_tN` = `mithril run --threads N`; `fuelN` = the built
program at `--threads 4 --fuel N`; `device` = `build --gpu` + `exec`.

| # | program | lanes | reference prints | correct |
|---|---|---|---|---|
| 1 | `fixture/mithril-codegen/net_array_tuples` | all 8 compiled lanes (CPU and device) | `4.77830972673649e-299` | `10` (oracle; by hand: `(3*3, 1)[0] + 1`). The net builds an array of tuples that compiled code reads with the wrong representation (design.md section 12; the codegen test is `#[ignore]`d). |
| 2 | `known/v2_shared` | all 8 compiled lanes | `(4.75, 4.75)` | `(4.75, 3.75)` (oracle, by hand). A float shared by compiled code and a closure is freed by the net's OP rule without its refcount (review axis 2). |
| 3 | `known/flo_share` | run_t1/t4/t16, fuel7, fuel64 | `(3.5e-323, 9.5, 0.25)` | `(4.75, 9.5, 4.0)` (oracle, by hand). Same bug as 2, read after free. |
| | | fuel1, fuel2 | `(6.4e-323, 9.5, 0.25)` | |
| | | device | `(1e-323, 9.5, 0.25)` | |
| 4 | `known/con_share_bug` | run_t1/t4/t16, fuel7, fuel64 | `(31, 36, 150)` | `(31, 60, 150)` (oracle, by hand). A list shared by compiled code and a closure under a conditional is freed by the net without its refcount (absorb front-core). The device lane is correct. |
| | | fuel1, fuel2 | `(31, 48, 150)` | |
| 5 | `known/con_share3` | all 8 compiled lanes | exit 101: specializer ICE `mithril-net/src/reduce.rs:217` "parked arm slot holds Var" | `(61, 60, 150)` (oracle, by hand). |
| 6 | `known/p6` | device | no result in 60 s (timeout) | `0`: lazy semantics. The closure `lambda x: x + spin(0)` is built but never applied (`y = big(30000) = 0`); `spin(0)` diverges, so any lane that evaluates it hangs. The oracle and every CPU lane print `0`. |
| 7 | `general/interp_closure` | fuel1, fuel2 | exit 101: runtime ICE "Op operand has non-value tag Era" | `240264` (oracle; every other lane agrees). |
| 8 | `lockless/histogram` | fuel2, fuel7, fuel64 | SIGSEGV (exit -11) | `3454372420` (`bench/lockless/small.json`; every twin and every other compiled lane). Seen deterministic under the harness (24 runs); a hand run at fuel 64 once completed, so treat it as schedule-dependent. |

## Oracle resource limits (not wrong answers)

The oracle lane runs under a 6 GB virtual-memory cap (`check.py`,
`ORACLE_MEM_KB`). The oracle copies an array on every update, so two corpus
programs exceed it and the oracle lane aborts (exit -6, "memory allocation
... failed"):

| program | uncapped oracle | correct (known.json) |
|---|---|---|
| `lockless/histogram` | more than 16 GB within 6 s (killed) | `3454372420` |
| `general/graph_dfs` | `3909815682`, 27 GB peak RSS, 19 s | `3909815682` |

## Device coverage

The device lane ran every program in the corpus: no program is rejected by
`build --gpu` (constant programs are artefacts of the form `MITHRIL-CONST v`).
The only device lane without a result is `known/p6` (row 6).

## Not covered by the corpus

- The compile-time reducer runs with fixed fuel (2^20) inside the CLI and no
  CLI flag changes it, so programs whose whole value is input-free (the generated
  programs, some fixtures) fold to a constant at compile time: their
  `run`, `fuel` and `device` lanes check the compile-time net reducer and the
  constant path, not the runtime engine. The codegen tests that force the
  runtime engine (reduce fuel 0) have no CLI equivalent.
- Budgets are the built program's `--fuel` flag; no environment variable
  sets them.
- Measured: 40 of the first 40 generated programs and 5 of the 57 fixtures
  build to a constant artefact.
