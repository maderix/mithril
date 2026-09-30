# bench/lockless: shared-state problems without locks in the source

Five programs whose usual parallel C and Rust versions update shared state
with locks, atomics or a reduction whose result depends on the schedule.
Each program has a Mithril implementation and C and Rust twins; for the
problems that also have a standard lock-free design in C and Rust, that
design is a second twin. Every implementation prints the same final
checksum line (fsum's idiomatic reductions excepted, see below):

* `<name>/main.py`: Mithril. No lock, atomic or shared mutable cell exists
  in the language; the parallelism is the forks of recursive folds.
* `<name>/main.c`: C11 with OpenMP and pthreads, updating one shared
  structure (atomics or locks), or for fsum the idiomatic OpenMP
  reduction. Build: `gcc -O2 -fopenmp -pthread`.
* `<name>/main_fold.c` (histogram, hashbuild): C, per-thread private
  structures merged at the end. `<name>/main_tree.c` (fsum): C, a
  fixed-shape tree sum.
* `rust/src/bin/<name>.rs`: Rust with rayon, updating one shared structure
  (`Mutex` or atomics), or for fsum rayon's `sum`. `rust/` is its own cargo
  package, outside the root workspace.
* `rust/src/bin/<name>_fold.rs` (histogram, hashbuild): rayon `fold` into
  thread-local structures, `reduce` to merge them.
  `rust/src/bin/fsum_tree.rs`: a fixed-shape tree sum.

These are Mithril's own benchmarks (spec: `docs/superpowers/specs/
2026-09-30-lockless-proof-evidence-design.md`, section 6), not reference ports.

| program | big size | C / Rust, shared structure or idiomatic reduction (`main.c`, `<name>.rs`) | C / Rust, fold/merge or tree (`main_fold.c`/`main_tree.c`, `<name>_fold.rs`/`fsum_tree.rs`) | Mithril uses |
|---|---|---|---|---|
| histogram | 2^24 keys into 2^16 bins | `omp atomic` increment / `AtomicU32::fetch_add` per key | OpenMP `reduction(+:bins[0:BINS])` (private bins per thread, combined at the end) / rayon `fold` into local bins, `reduce` adds them | a fold: each leaf counts 2^16 keys into its own bins, halves fork and add bin by bin |
| hashbuild | 2^20 keys (20+4 bits, some repeat) into 2^20 slots | 4096 striped `pthread_mutex` / a `Mutex` per slot, chained buckets | per-thread chained tables, then a parallel merge by slot (each thread owns a slot range; no lock) / rayon `fold` into local `HashMap`s, `reduce` unions them | an immutable trie over the slot bits, built by a fold; merges fork and union the chains |
| bfs | 2^20 nodes, out-degree 8, from node 0 | CAS on each node's level word, atomic frontier tail / CAS, rayon collect | - | level sets as values: tries over the node bits; expand, difference and union all fork |
| components | 2^20 nodes, 2^20 undirected edges | lock-free union-find (CAS link to the smaller root, CAS path halving) | - | label propagation to a fixpoint: min over x in {v} and v's neighbours of lab[lab[x]]; the labels array is read only during a round, the round's changes come back as a tree |
| fsum | 2^24 f32 values in [-0.5, 0.5) | OpenMP `reduction(+:s)` / rayon `sum` | 2^8 aligned chunks summed by the halving tree in parallel, partials added by the same tree | a fixed-shape tree sum (halves fork) |

histogram and hashbuild are solved in C and Rust both ways: with a shared
structure and synchronization in the source (`main.c`, `<name>.rs`), and
with the standard lock-free fold/merge design (`main_fold.c`,
`<name>_fold.rs`), which is the same shape as the Mithril program. In
Mithril the fold is the only form: with no shared mutable cell there is
no shared-structure version to write, and the synchronization the fold
needs (joining the forks) is the runtime's.
bfs and components have no fold/merge twin here: their C and Rust
versions are the usual lock-free CAS designs.

Checksums (`expected.txt`, big size; `small.json`, small size):

* histogram: sum over bins of mix(bin, count), u32.
* hashbuild: (occupied slots, sum over occupied slots of mix(slot, sum of
  keys + count * 2654435761)), u32. The per-slot summary does not depend
  on the order keys arrived in, so the locked and fold/merge twins agree
  with Mithril.
* bfs: (nodes reached, sum of their levels).
* components: (components, sum over nodes of mix(node, component minimum
  id)), u32.
* fsum: the f32 sum's bit pattern as an unsigned integer. Mithril's tree
  shape depends only on the size, so its bits are the same at every thread
  count: small 3231587146 (-4.940831), big 3232996432 (-5.612831), checked
  against a numpy pairwise sum of the same shape. `main_tree.c` and
  `fsum_tree.rs` use the same tree (2^8 aligned chunks, each a halving
  tree, then the halving tree over the partials) and print the same bits
  at every thread count. The nondeterminism of `main.c` and `fsum.rs` comes
  from their idiomatic reductions, not from C or Rust: the OpenMP
  reduction combines per-thread partial sums in arrival order and rayon's
  `sum` splits adaptively, so their bits vary with thread count and from
  run to run. A fixed-shape tree reduction is deterministic in any
  language; Mithril's difference is that its fold has no other shape.

## Running

Sizes: every source has one line marked `SIZE` (`def size()` in
Mithril, `#define LOG` in C, `const LOG` in Rust). `small.json` gives,
per program, a sed expression that rewrites it (the same expression for
every source of the program) and the small checksum. The small sizes
(histogram 2^20, fsum 2^18, the others 2^12) are chosen so that every
Mithril program forks at `--threads 4` (`MITHRIL_STATS=1` reports
waves > 0: histogram 4, hashbuild 7, bfs 23, components 7, fsum 7) while
running well under a second; at smaller sizes histogram and fsum run as
one sequential rewrite and the determinism test would not exercise the
parallel path.

    python3 bench/lockless/sync_count.py
    python3 bench/lockless/determinism.py --runs 100 --size small
    python3 bench/lockless/determinism.py --runs 100 --size big --threads 1,4,16

* `sync_count.py` counts synchronization operation sites in each
  implementation's source (outside comments), with one rule for C and
  Rust: a site is a place where the program performs a synchronizing
  operation on shared memory. C: `atomic_*` calls, `__atomic`/`__sync`
  builtins, pthread lock/unlock/wait calls, `#pragma omp atomic|critical|
  barrier|flush|ordered`, and each `reduction(` clause (GCC lowers the
  combine into the shared variable to a `lock cmpxchg` loop for a scalar
  and to a `GOMP_atomic_start`/`end` region for an array section). Rust:
  `.lock()` (and the RwLock `.read()`/`.write()`), and the atomic methods
  `.load`, `.store`, `.swap`, `.fetch_*`, `.compare_exchange*`. Types,
  declarations, `sizeof(_Atomic ...)` and `use` lines are not sites;
  creating a lock or atomic (`atomic_init`, `pthread_mutex_init`,
  `Mutex::new`, `Atomic*::new`) is reported apart as `init`. Fork/join
  structure is not counted in any language (the implicit barrier of
  `omp parallel for`, rayon `join`/`fold`/`reduce`/`sum`/`collect`, and
  Mithril's forks). Mithril is 0 by construction. It prints one row per
  implementation, with lines of code.
* The counts are of program source. Every implementation synchronizes
  underneath: libgomp and rayon use atomics and locks internally, and so
  does Mithril's runtime, which joins forks and hands out work with
  atomics (join counters, wave claim cursors, refcounts:
  `crates/mithril-rt/src/sync.rs`, model-checked with loom). What differs
  is where it lives: in the C and Rust shared-structure twins the program
  writes it; in the fold/merge twins and in Mithril it is inside the
  fork/join machinery.
* `determinism.py` builds each implementation (Mithril with
  `target/release/mithril build main.py -o <out>`, run with
  `--threads T`; C with `gcc -O2 -fopenmp -pthread`, `OMP_NUM_THREADS=T`;
  Rust with `cargo build --release`, `RAYON_NUM_THREADS=T`), runs it N
  times at each thread count, compares the whole stdout byte for byte and
  prints the number of distinct outputs and whether the checksum line is
  the expected one. Variants are found by file name and labelled `c`,
  `c_fold`, `c_tree`, `rust`, `rust_fold`, `rust_tree`; `--impl c` selects
  every C variant. A run that times out, or a build that fails, is
  recorded as a failure of that row and the script goes on. Small builds
  go to `bench/lockless/.build/`. It never uses the GPU lane; the device
  lane of the spec's protocol is run separately.

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
