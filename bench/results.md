# Mithril benchmark results (CPU, BIG size)

- Git HEAD: `2a3f23e80baa09499954ed64c2ab8fc27239d47c` (branch `mithril-v1`)
- CPU: AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs)
- RAM: 62.0 GiB; kernel 6.17.4-76061704-generic
- rustc: rustc 1.86.0 (05f9846f8 2025-03-31)
- gcc: gcc (Ubuntu 11.4.0-1ubuntu1~22.04.2) 11.4.0
- Date: 2026-09-29 23:41:03 IST

Lanes: `C` = C twin `gcc -O2`; `SEQ` = Mithril program at `--threads 1`; `PAR16` = `--threads 16`. Mithril programs are built once with `mithril build` (the same pipeline `mithril run` uses; build time in notes, excluded from lane times). Times are wall-clock minimum of 3 runs per lane. Per-run timeout 300s -> `DNF`. Ratios are Mithril / C (lower is better). Every completed run's stdout was checked against `bench/expected.txt`; `FAILED` = wrong checksum or crash. Mithril env: MITHRIL_STATS unset, default arenas (retry with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28 on `arena exhausted`, noted).

| name | C | SEQ | PAR16 | SEQ/C | PAR16/C | DNF / notes |
|---|---|---|---|---|---|---|
| bfs | - | - | - | - | - |  |
| editdist | - | - | - | - | - |  |
| gameoflife | - | - | - | - | - |  |
| hashmap | - | - | - | - | - |  |
| kdtree | - | - | - | - | - |  |
| kmeans | - | - | - | - | - |  |
| lexer | - | - | - | - | - |  |
| mandelbrot | - | - | - | - | - |  |
| merkle | - | - | - | - | - |  |
| nbody | - | - | - | - | - |  |
| queens | - | - | - | - | - |  |
| raytrace | - | - | - | - | - |  |
| symreg | - | - | - | - | - |  |
| terrain | - | - | - | - | - |  |
| tree-bitonic | - | - | - | - | - |  |
| tree-matmul | - | - | - | - | - |  |
| tree-radix | - | - | - | - | - |  |
