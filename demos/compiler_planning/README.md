# Compiler decisions in an interaction net

One small graph, four resource contracts, several reduction orders. The net
jointly chooses fusion, vector width, tile size, scratch buffers and placement.
The viewer shows the selected plans and replays actual net reductions.

From the repository root:

```sh
pwd; python3 demos/compiler_planning/demo.py --out target/compiler-planning/demo
```

Open `target/compiler-planning/demo/demo.html`. The generated page is standalone.
Use the scenario buttons to change the resource contract, and select **Resume
saved net** to compare continuations from the same partially evaluated state.
Every captured answer must match the Core interpreter and an independent
exhaustive reference.

The CPU and accelerator costs are a synthetic contract. The demo executes
compiler decisions through Mithril's shared rule table. It does not run an
accelerator kernel. Cell counts measure occupied net slots, excluding host
allocator capacity and queue storage.

Files:

- `planner.mithril`: summaries, legality, storage and placement search.
- `oracle.py`: independent exhaustive reference and input contract.
- `runtime.rs`: reduction order, traces and copied-state continuation runner.
- `demo.py`: input marshaling, execution, checks and page generation.
- `view.html`: replay viewer.
- `test_*.py`: compiler and demo regression tests.

Checks:

```sh
pwd; python3 -m unittest discover -s demos/compiler_planning -p 'test_*.py' -v
pwd; cargo test --release -p mithril-net --example compiler_planning
```

Earlier proof experiments were archived on 2026-10-01:

- Git snapshot: `archive/compiler-proofs-2026-10-01`, commit `de22f34`.
- Sources and receipts: `target/archives/compiler-proofs-2026-10-01.tar.gz`.
- Per-file hashes: the adjacent `.tar.manifest.json`.

Extract the archive into a separate directory to inspect the old files. It
contains original repository-relative paths; no files need to be restored over
the current checkout.
