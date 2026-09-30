# bench/lockless: programs that need locks elsewhere

Five programs whose usual parallel C and Rust versions need locks, atomics
or a reduction whose result depends on the schedule. Each has three
implementations that print the same final checksum line (fsum excepted,
see below):

* `<name>/main.py`: Mithril. No lock, atomic or shared mutable cell exists
  in the language; the parallelism is the forks of recursive folds.
* `<name>/main.c`: C11 with OpenMP and pthreads, using the synchronization
  the problem idiomatically needs. Build: `gcc -O2 -fopenmp -pthread`.
* `rust/src/bin/<name>.rs`: Rust with rayon and `Mutex` or atomics as
  idiomatic. `rust/` is its own cargo package, outside the root workspace.

These are Mithril's own benchmarks (spec: `docs/superpowers/specs/
2026-09-30-lockless-proof-evidence-design.md`, section 6), not reference ports.

| program | big size | C / Rust use | Mithril uses |
|---|---|---|---|
| histogram | 2^24 keys into 2^16 bins | `omp atomic` increment / `AtomicU32::fetch_add` per key | a fold: each leaf counts 2^16 keys into its own bins, halves fork and add bin by bin |
| hashbuild | 2^20 keys (20+4 bits, some repeat) into 2^20 slots | 4096 striped `pthread_mutex` / a `Mutex` per slot, chained buckets | an immutable trie over the slot bits, built by a fold; merges fork and union the chains |
| bfs | 2^20 nodes, out-degree 8, from node 0 | CAS on each node's level word, atomic frontier tail / CAS, rayon collect | level sets as values: tries over the node bits; expand, difference and union all fork |
| components | 2^20 nodes, 2^20 undirected edges | lock-free union-find (CAS link to the smaller root, CAS path halving) | label propagation to a fixpoint: min over x in {v} and v's neighbours of lab[lab[x]]; the labels array is read only during a round, the round's changes come back as a tree |
| fsum | 2^24 f32 values in [-0.5, 0.5) | OpenMP `reduction(+:s)` / rayon `sum` | a fixed-shape tree sum (halves fork) |

Checksums (`expected.txt`, big size; `small.json`, small size):

* histogram: sum over bins of mix(bin, count), u32.
* hashbuild: (occupied slots, sum over occupied slots of mix(slot, sum of
  keys + count * 2654435761)), u32. The per-slot summary does not depend
  on the order keys arrived in, so the locked twins agree with Mithril.
* bfs: (nodes reached, sum of their levels).
* components: (components, sum over nodes of mix(node, component minimum
  id)), u32.
* fsum: the f32 sum's bit pattern as an unsigned integer. Mithril's tree
  shape depends only on the size, so its bits are the same at every thread
  count: small 3224226931 (-2.7156036), big 3232996432 (-5.612831),
  checked against a numpy pairwise sum of the same shape. The C reduction
  combines per-thread partial sums in arrival order and rayon splits
  adaptively, so their bits vary with thread count and from run to run.
  This is the expected difference, not a bug in the twins.

## Running

Sizes: every source has one line marked `SIZE` (`def size()` in
Mithril, `#define LOG` in C, `const LOG` in Rust). `small.json` gives,
per program, a sed expression that rewrites it (the same expression for
all three sources) and the small checksum.

    python3 bench/lockless/sync_count.py
    python3 bench/lockless/determinism.py --runs 100 --size small
    python3 bench/lockless/determinism.py --runs 100 --size big --threads 1,4,16

* `sync_count.py` counts synchronization primitives in each source
  (outside comments): C `pthread_*` locks, `_Atomic`, `atomic_*`,
  `__sync`/`__atomic`, `#pragma omp atomic|critical|barrier`, `reduction(`;
  Rust `Mutex`, `RwLock`, `Atomic*`, `fetch_*`, `compare_exchange*`.
  Mithril is 0 by construction. Library-internal synchronization (libgomp,
  rayon's work stealing, the implicit barrier of `omp parallel for`) is not
  counted, so Rust fsum shows 0 while still being schedule-dependent. It
  also prints lines of code.
* `determinism.py` builds each program (Mithril with
  `target/release/mithril build main.py -o <out>`, run with
  `--threads T`; C with `gcc -O2 -fopenmp -pthread`, `OMP_NUM_THREADS=T`;
  Rust with `cargo build --release`, `RAYON_NUM_THREADS=T`), runs it N
  times at each thread count, compares the whole stdout byte for byte and
  prints the number of distinct outputs and whether the checksum line is
  the expected one. Small builds go to `bench/lockless/.build/`. It never
  uses the GPU lane; the device lane of the spec's protocol is run
  separately.

Timing is not done here. It is done by a later script under the same
conditions as `docs/design.md` section 9 (same machine, load below 2, one
warm-up, minimum of 3, at t1, t8 pinned and t16, and the device lane).

## Notes on the Mithril sources

* histogram's leaf grain (2^16 keys, the bin count) makes the merge work
  equal to the counting work; it is the natural size for a per-leaf bin
  array, not a tuned constant.
* components uses label propagation with one pointer jump per round
  (lab[lab[x]]), which converges in far fewer rounds than plain
  propagation and keeps the fixpoint (every label is a node of the same
  component and never above the node's id, so the fixpoint label is the
  component minimum). The round's writes are applied by one sequential
  pass over the change tree; the read side is the parallel part.
* No program needed a compiler change. `min` is not a builtin, so
  components defines `least`.
