# Mithril benchmark results (CPU, BIG size)

- Git HEAD: `620e689455b1779c1abc631f1870857bc24eda24` (branch `mithril-v1`)
- CPU: AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs)
- RAM: 62.0 GiB; kernel 6.17.4-76061704-generic
- rustc: rustc 1.86.0 (05f9846f8 2025-03-31)
- gcc: gcc (Ubuntu 11.4.0-1ubuntu1~22.04.2) 11.4.0
- Date: 2026-09-27 20:18:36 IST

Lanes: `C` = C twin `gcc -O2`; `SEQ` = Mithril program at `--threads 1`; `PAR16` = `--threads 16`. Mithril programs are built once with `mithril build` (the same pipeline `mithril run` uses; build time in notes, excluded from lane times). Times are wall-clock minimum of 1 runs per lane. Per-run timeout 300s -> `DNF`. Ratios are Mithril / C (lower is better). Every completed run's stdout was checked against `bench/expected.txt`; `FAILED` = wrong checksum or crash. Mithril env: MITHRIL_STATS unset, default arenas (retry with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28 on `arena exhausted`, noted).

| name | C | SEQ | PAR16 | SEQ/C | PAR16/C | DNF / notes |
|---|---|---|---|---|---|---|
| bfs **FAILED** | 4.624s | DNF | FAILED | - | - | build 0.8s; C n=1; SEQ DNF (>300s); SEQ: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28; PAR16: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28 |
| editdist **FAILED** | 2.257s | DNF | FAILED | - | - | build 0.9s; C n=1; SEQ DNF (>300s); SEQ: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28; PAR16: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28 |
| gameoflife | 28.875s | 140.066s | 17.826s | 4.85x | 0.62x | build 1.0s; C n=1; SEQ n=1; PAR16 n=1 |
| hashmap | 0.796s | 57.232s | 6.083s | 71.91x | 7.64x | build 0.8s; C n=1; SEQ: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28; SEQ n=1; PAR16: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28; PAR16 n=1 |
| kmeans | 10.324s | 64.147s | 6.866s | 6.21x | 0.67x | build 2.1s; C n=1; SEQ n=1; PAR16 n=1 |
| lexer | 1.131s | 221.073s | 29.794s | 195.52x | 26.35x | build 0.7s; C n=1; SEQ n=1; PAR16 n=1 |
| mandelbrot | 1.996s | 5.718s | 0.649s | 2.86x | 0.33x | build 1.0s; C n=1; SEQ n=1; PAR16 n=1 |
| merkle | 5.860s | 15.153s | 5.214s | 2.59x | 0.89x | build 0.7s; C n=1; SEQ: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28; SEQ n=1; PAR16: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28; PAR16 n=1 |
| nbody **FAILED** | FAILED | DNF | DNF | - | - | build 9.2s; SEQ DNF (>300s); SEQ: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28; PAR16 DNF (>300s); PAR16: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28 |
| queens | 3.961s | 196.261s | 30.306s | 49.55x | 7.65x | build 0.6s; C n=1; SEQ n=1; PAR16 n=1 |
| raytrace | 9.096s | DNF | DNF | - | - | build 2.9s; C n=1; SEQ DNF (>300s); PAR16 DNF (>300s); PAR16: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28 |
| symreg | 2.993s | 32.382s | 4.358s | 10.82x | 1.46x | build 0.8s; C n=1; SEQ n=1; PAR16 n=1 |
| terrain | 4.223s | DNF | DNF | - | - | build 0.9s; C n=1; SEQ DNF (>300s); PAR16 DNF (>300s) |
| tree-bitonic | 8.507s | 260.575s | 56.831s | 30.63x | 6.68x | build 0.7s; C n=1; SEQ: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28; SEQ n=1; PAR16: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28; PAR16 n=1 |
| tree-matmul | 4.258s | 98.759s | 13.808s | 23.19x | 3.24x | build 1.0s; C n=1; SEQ: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28; SEQ n=1; PAR16: arena exhausted at defaults; retried with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28; PAR16 n=1 |

## Failures / DNFs

- `bfs` SEQ DNF: timeout after 300s (run 1)
- `bfs` PAR16 FAILED: exit 101 (run 1); stdout=''; stderr tail=" | thread 'mithril-worker' panicked at /home/maderix/Mithril/crates/mithril-rt/src/alloc.rs:114:13: | arena exhausted: cell arena full at 4294967296 slots (raise MITHRIL_NODES, max 4294967296 = 2^32, e.g. MITHRIL_NODES=1<<28)"
- `editdist` SEQ DNF: timeout after 300s (run 1)
- `editdist` PAR16 FAILED: exit 101 (run 1); stdout=''; stderr tail=" | thread 'mithril-worker' panicked at /home/maderix/Mithril/crates/mithril-rt/src/alloc.rs:114:13: | arena exhausted: cell arena full at 4294967296 slots (raise MITHRIL_NODES, max 4294967296 = 2^32, e.g. MITHRIL_NODES=1<<28)"
- `nbody` C FAILED: gcc -O2 failed: /home/maderix/Mithril/bench/ports/nbody.c: In function ‘loop3’:
/home/maderix/Mithril/bench/ports/nbody.c:114:29: error: expected identifier or ‘(’ before ‘return’
  114 |   __attribute__((musttail)) return loop3(s - 1, x0 + nvx0 * 0.001f, y0 + nvy0 * 0.001f,
      |                             ^~~~~~
- `nbody` SEQ DNF: timeout after 300s (run 1)
- `nbody` PAR16 DNF: timeout after 300s (run 1)
- `raytrace` SEQ DNF: timeout after 300s (run 1)
- `raytrace` PAR16 DNF: timeout after 300s (run 1)
- `terrain` SEQ DNF: timeout after 300s (run 1)
- `terrain` PAR16 DNF: timeout after 300s (run 1)
