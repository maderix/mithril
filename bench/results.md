# Mithril benchmark results (CPU, BIG size)

- Git HEAD: `7b6e11384cf6bb8ca0d94d1e2dc9632b9a202534` (branch `mithril-v1`)
- CPU: AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs)
- RAM: 62.0 GiB; kernel 6.17.4-76061704-generic
- rustc: rustc 1.86.0 (05f9846f8 2025-03-31)
- gcc: gcc (Ubuntu 11.4.0-1ubuntu1~22.04.2) 11.4.0
- Date: 2026-09-29 23:10:26 IST

Lanes: `C` = C twin `gcc -O2`; `SEQ` = Mithril program at `--threads 1`; `PAR16` = `--threads 16`. Mithril programs are built once with `mithril build` (the same pipeline `mithril run` uses; build time in notes, excluded from lane times). Times are wall-clock minimum of 3 runs per lane. Per-run timeout 300s -> `DNF`. Ratios are Mithril / C (lower is better). Every completed run's stdout was checked against `bench/expected.txt`; `FAILED` = wrong checksum or crash. Mithril env: MITHRIL_STATS unset, default arenas (retry with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28 on `arena exhausted`, noted).

| name | C | SEQ | PAR16 | SEQ/C | PAR16/C | DNF / notes |
|---|---|---|---|---|---|---|
| bfs | 4.476s | 4.834s | 0.432s | 1.08x | 0.10x | build 0.6s; C n=3; SEQ n=3; PAR16 n=3 |
| editdist | 2.173s | 2.248s | 0.255s | 1.03x | 0.12x | build 0.5s; C n=3; SEQ n=3; PAR16 n=3 |
| gameoflife | 28.324s | 8.449s | 1.121s | 0.30x | 0.04x | build 1.0s; C n=3; SEQ n=3; PAR16 n=3 |
| hashmap | 0.767s | 1.823s | 0.237s | 2.38x | 0.31x | build 0.7s; C n=3; SEQ n=3; PAR16 n=3 |
| kdtree | 0.333s | 0.406s | 0.090s | 1.22x | 0.27x | build 1.0s; C n=3; SEQ n=3; PAR16 n=3 |
| kmeans | 10.164s | 11.856s | 1.462s | 1.17x | 0.14x | build 1.1s; C n=3; SEQ n=3; PAR16 n=3 |
| lexer | 1.033s | 2.514s | 0.347s | 2.43x | 0.34x | build 0.9s; C n=3; SEQ n=3; PAR16 n=3 |
| mandelbrot | 1.919s | 4.078s | 0.850s | 2.12x | 0.44x | build 1.0s; C n=3; SEQ n=3; PAR16: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28; PAR16 n=3 |
| merkle | 5.754s | 5.433s | 0.601s | 0.94x | 0.10x | build 0.9s; C n=3; SEQ n=3; PAR16 n=3 |
| nbody **FAILED** | FAILED | 5.823s | 0.524s | - | - | build 0.8s; SEQ n=3; PAR16 n=3 |
| queens | 3.904s | 4.707s | 0.454s | 1.21x | 0.12x | build 0.5s; C n=3; SEQ n=3; PAR16 n=3 |
| raytrace | 9.031s | 7.681s | 0.849s | 0.85x | 0.09x | build 1.4s; C n=3; SEQ n=3; PAR16 n=3 |
| symreg | 3.011s | 3.016s | 0.400s | 1.00x | 0.13x | build 0.7s; C n=3; SEQ n=3; PAR16 n=3 |
| terrain | 4.125s | 4.579s | 0.559s | 1.11x | 0.14x | build 0.6s; C n=3; SEQ n=3; PAR16 n=3 |
| tree-bitonic | 8.503s | 10.792s | 1.554s | 1.27x | 0.18x | build 0.6s; C n=3; SEQ n=3; PAR16 n=3 |
| tree-matmul | 4.202s | 3.798s | 0.575s | 0.90x | 0.14x | build 2.1s; C n=3; SEQ n=3; PAR16 n=3 |
| tree-radix | 2.574s | 4.052s | 0.579s | 1.57x | 0.23x | build 0.6s; C n=3; SEQ n=3; PAR16 n=3 |

## Failures / DNFs

- `nbody` C FAILED: gcc -O2 failed: /home/maderix/Mithril/bench/ports/nbody.c: In function ‘loop3’:
/home/maderix/Mithril/bench/ports/nbody.c:114:29: error: expected identifier or ‘(’ before ‘return’
  114 |   __attribute__((musttail)) return loop3(s - 1, x0 + nvx0 * 0.001f, y0 + nvy0 * 0.001f,
      |                             ^~~~~~
