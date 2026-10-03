# Mithril benchmark results (CPU, BIG size)

- Git HEAD: `61702e101d112cd9b7e89ef7121d52c079f05ecf` (branch `master`)
- CPU: AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs)
- RAM: 62.0 GiB; kernel 6.17.4-76061704-generic
- rustc: rustc 1.86.0 (05f9846f8 2025-03-31)
- gcc: gcc (Ubuntu 11.4.0-1ubuntu1~22.04.2) 11.4.0
- Date: 2026-10-03 10:19:38 IST

Lanes: `C` = C twin, the faster of `gcc -O3` and `clang -O3` (default target, as Mithril's `rustc -O`); `SEQ` = Mithril program at `--threads 1`; `PAR16` = `--threads 16`. Mithril programs are built once with `mithril build` (the same pipeline `mithril run` uses; build time in notes, excluded from lane times). Times are wall-clock minimum of adaptive (3 if first run < 60s, else 1) runs per lane. Per-run timeout 300s -> `DNF`. Ratios are Mithril / C (lower is better). Every completed run's stdout was checked against `bench/expected.txt`; `FAILED` = wrong checksum or crash. Mithril env: MITHRIL_STATS unset, default arenas (retry with MITHRIL_NODES=1<<32 MITHRIL_RECS=1<<28 on `arena exhausted`, noted).

| name | C | SEQ | PAR16 | SEQ/C | PAR16/C | DNF / notes |
|---|---|---|---|---|---|---|
| collatz | 0.345s | 0.657s | 0.057s | 1.91x | 0.16x | build 0.5s; C: gcc -O3; C n=3; SEQ n=3; PAR16 n=3 |
| heat2d | 0.522s | 1.405s | 1.408s | 2.69x | 2.70x | C: kept from 61702e1; C n=3; SEQ: kept from 61702e1; SEQ n=3; PAR16: kept from 61702e1; PAR16 n=3 |
| histogram | 0.380s | 1.301s | 0.168s | 3.42x | 0.44x | C: kept from 61702e1; C n=3; SEQ: kept from 61702e1; SEQ n=3; PAR16: kept from 61702e1; PAR16 n=3 |
| kdtree | 0.357s | 0.389s | 0.088s | 1.09x | 0.25x | C: kept from 61702e1; C n=3; SEQ: kept from 61702e1; SEQ n=3; PAR16: kept from 61702e1; PAR16 n=3 |
| knapsack | 0.350s | 0.558s | 0.549s | 1.60x | 1.57x | C: kept from 61702e1; C n=3; SEQ: kept from 61702e1; SEQ n=3; PAR16: kept from 61702e1; PAR16 n=3 |
| msort | 0.393s | 0.284s | 0.120s | 0.72x | 0.30x | C: kept from 61702e1; C n=3; SEQ: kept from 61702e1; SEQ n=3; PAR16: kept from 61702e1; PAR16 n=3 |
| subsetsum | 0.545s | 1.615s | 1.106s | 2.96x | 2.03x | C: kept from 61702e1; C n=3; SEQ: kept from 61702e1; SEQ n=3; PAR16: kept from 61702e1; PAR16 n=3 |
