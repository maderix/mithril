# Mithril benchmark results (CPU, BIG size)

- Git HEAD: `15f51dd97461f01564662d327165150b9a2b7bc6` (branch `mithril-v1`)
- CPU: AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs)
- RAM: 62.0 GiB; kernel 6.17.4-76061704-generic
- rustc: rustc 1.86.0 (05f9846f8 2025-03-31)
- gcc: gcc (Ubuntu 11.4.0-1ubuntu1~22.04.2) 11.4.0
- Date: 2026-09-29 19:11:46 IST

Lanes: `C` = C twin `gcc -O2`; `SEQ` = Mithril program at `--threads 1`; `PAR16` = `--threads 16`. Mithril programs are built once with `mithril build` (the same pipeline `mithril run` uses; build time in notes, excluded from lane times). Times are wall-clock minimum of 1 runs per lane. Per-run timeout 300s -> `DNF`. Ratios are Mithril / C (lower is better). Every completed run's stdout was checked against `bench/expected.txt`; `FAILED` = wrong checksum or crash. Mithril env: MITHRIL_STATS unset, default arenas (retry with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28 on `arena exhausted`, noted).

| name | C | SEQ | PAR16 | SEQ/C | PAR16/C | DNF / notes |
|---|---|---|---|---|---|---|
| bfs | 4.562s | 4.870s | 0.428s | 1.07x | 0.09x | build 0.6s; C n=1; SEQ n=1; PAR16 n=1 |
| editdist | 2.201s | 2.452s | 0.252s | 1.11x | 0.11x | build 0.5s; C n=1; SEQ n=1; PAR16 n=1 |
| gameoflife | 28.290s | 8.462s | 8.454s | 0.30x | 0.30x | build 1.0s; C n=1; SEQ n=1; PAR16 n=1 |
| hashmap | 0.769s | 1.833s | 0.226s | 2.38x | 0.29x | build 0.7s; C n=1; SEQ n=1; PAR16 n=1 |
| kdtree | 0.341s | 0.416s | 0.109s | 1.22x | 0.32x | build 1.0s; C n=1; SEQ n=1; PAR16 n=1 |
| kmeans | 10.176s | 11.912s | 1.475s | 1.17x | 0.14x | build 1.1s; C n=1; SEQ n=1; PAR16 n=1 |
| lexer | 1.067s | 2.524s | 0.358s | 2.37x | 0.34x | build 0.9s; C n=1; SEQ n=1; PAR16 n=1 |
| mandelbrot | 1.949s | 3.553s | 0.827s | 1.82x | 0.42x | build 0.9s; C n=1; SEQ n=1; PAR16: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28; PAR16 n=1 |
| merkle | 5.776s | 5.402s | 0.612s | 0.94x | 0.11x | build 0.9s; C n=1; SEQ n=1; PAR16 n=1 |
| nbody **FAILED** | FAILED | 5.826s | 5.823s | - | - | build 0.8s; SEQ n=1; PAR16 n=1 |
| queens | 3.913s | 4.806s | 4.787s | 1.23x | 1.22x | build 0.5s; C n=1; SEQ n=1; PAR16 n=1 |
| raytrace | 9.042s | 7.693s | 0.852s | 0.85x | 0.09x | build 1.4s; C n=1; SEQ n=1; PAR16 n=1 |
| symreg | 2.956s | 3.103s | 3.210s | 1.05x | 1.09x | build 0.7s; C n=1; SEQ n=1; PAR16 n=1 |
| terrain | 4.170s | 4.591s | 0.560s | 1.10x | 0.13x | build 0.6s; C n=1; SEQ n=1; PAR16 n=1 |
| tree-bitonic | 8.464s | 10.749s | 1.584s | 1.27x | 0.19x | build 0.6s; C n=1; SEQ n=1; PAR16 n=1 |
| tree-matmul | 4.236s | 3.678s | 0.541s | 0.87x | 0.13x | build 1.5s; C n=1; SEQ n=1; PAR16 n=1 |
| tree-radix | 2.671s | 3.950s | 0.589s | 1.48x | 0.22x | build 0.6s; C n=1; SEQ n=1; PAR16 n=1 |

## Failures / DNFs

- `nbody` C FAILED: gcc -O2 failed: /home/maderix/Mithril/bench/ports/nbody.c: In function ‘loop3’:
/home/maderix/Mithril/bench/ports/nbody.c:114:29: error: expected identifier or ‘(’ before ‘return’
  114 |   __attribute__((musttail)) return loop3(s - 1, x0 + nvx0 * 0.001f, y0 + nvy0 * 0.001f,
      |                             ^~~~~~
