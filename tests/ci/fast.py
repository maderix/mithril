#!/usr/bin/env python3
"""Fast regression gate for Mithril: catches the failure classes that cost
us days when only found by running the full benchmark suite.

For every port in bench/ports (all in parallel, memory-capped builds):

  correctness  small-size build, --threads 1 and 16, exact checksum
  lowering     the set of functions native-scalar-lowered (fn s_<id>) must
               not shrink (a function silently falling back to tagged dive
               code is a 5-30x sequential regression)
  code size    generated lines and segment count within tolerance
               (exponential continuation duplication shows up here)
  build time   rustc time on the generated program (soft)
  perf         instruction count (perf stat, noise-free) of a mid-size run
               within tolerance of baseline; t1 == t16 checksum on mid

Everything is compared against tests/ci/baseline.json. `--update` rewrites
the baseline from the current toolchain (do this only from a state you
have verified against reference / the expected checksums).

Usage: tests/ci/fast.py [--update] [--only NAME ...] [--jobs N]
Exit status: number of hard failures.
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PORTS = os.path.join(ROOT, "bench", "ports")
BASELINE = os.path.join(ROOT, "tests", "ci", "baseline.json")
TARGET = os.environ.get("CARGO_TARGET_DIR", os.path.join(ROOT, "target"))
BIN = os.environ.get("MITHRIL_BIN", os.path.join(TARGET, "release", "mithril"))
DUMP = os.path.join(TARGET, "release", "examples", "dump_gen")

BUILD_MEM_KB = 12_000_000
BUILD_CAP_S = 300
RUN_CAP_S = 60

# small size: (sed substitutions, expected checksum) — mirrors tests/e2e/run.sh
SMALL = {
    "bfs": (["s/batch(19, 0)/batch(4, 0)/"], "512822161"),
    "editdist": (["s/batch(15, 0)/batch(2, 0)/"], "2065873279"),
    "gameoflife": (["s/census_fin(batch_run(18, 0, 32))/census_fin(batch_run(1, 0, 2))/"], "2601177279"),
    "hashmap": (["s/batch(11, 0, 16384)/batch(2, 0, 256)/"], "4206244792"),
    "kmeans": (["s/rbatch(6, 0, 19)/rbatch(6, 0, 7)/"], "4219097976"),
    "lexer": (["s/batch(23, 0)/batch(8, 0)/"], "1822208108"),
    "mandelbrot": (["s/rend(18, 51)/rend(2, 7)/"], "887240761"),
    "merkle": (["s/build(22, 0)/build(4, 0)/", "s/pgen(22, 1337, 0), leafh(1337)/pgen(4, 13, 0), leafh(13)/"], "524568222"),
    "nbody": (["s/run(17, 300)/run(7, 300)/"], "2215450620"),
    "queens": (["s/run(17, 17, 11730)/run(10, 5, 625)/"], "774553824"),
    "raytrace": (["s/rowf(12, 0, 4095, 6000, 1161527296, 1157627904)/rowf(6, 0, 63, 80, 1109393408, 1107296256)/"], "402971"),
    "kdtree": (["s/    n = 18/    n = 10/", "s/return qbatch(t, 18, 0)/return qbatch(t, 10, 0)/"], "2478099586"),
    "symreg": (["s/run(18, 42, 32, 110)/run(6, 42, 32, 16)/"], "2490246820"),
    "terrain": (["s/for t in range(65536):/for t in range(16):/"], "4236200168"),
    "tree-bitonic": (["s/bsort(23, 0, 0)/bsort(8, 0, 0)/"], "971629740"),
    "tree-matmul": (["s/batch(9, 0, 384, 511, 7)/batch(2, 0, 3, 3, 3)/"], "4292995302"),
    "tree-radix": (["s/gen(22, 0)/gen(8, 0)/"], "366571299"),
}

# mid size for the perf signal (no expected value; t1 and t16 must agree)
MID = {
    "bfs": ["s/batch(19, 0)/batch(11, 0)/"],
    "editdist": ["s/batch(15, 0)/batch(7, 0)/"],
    "gameoflife": ["s/batch_run(18, 0, 32)/batch_run(12, 0, 32)/"],
    "hashmap": ["s/batch(11, 0, 16384)/batch(6, 0, 16384)/"],
    "lexer": ["s/batch(23, 0)/batch(16, 0)/"],
    "mandelbrot": ["s/rend(18, 51)/rend(10, 51)/"],
    "merkle": ["s/build(22, 0)/build(14, 0)/", "s/pgen(22, 1337, 0)/pgen(14, 1337, 0)/"],
    "nbody": ["s/run(17, 300)/run(8, 300)/"],
    "symreg": ["s/run(18, 42, 32, 110)/run(12, 42, 32, 110)/"],
    "terrain": ["s/for t in range(65536):/for t in range(1024):/"],
    "tree-bitonic": ["s/bsort(23, 0, 0)/bsort(16, 0, 0)/"],
    "tree-radix": ["s/gen(22, 0)/gen(16, 0)/"],
}

TOL = {"segments": 1.5, "gen_lines": 1.3, "instr": 1.15, "build_s": 2.5}
SOFT = {"build_s"}
# CPU lanes known to take minutes even at small size (float emulation);
# code metrics still gated, runs reported but not counted as failures
KNOWN_SLOW = set()
RUN_ENV = dict(os.environ, MITHRIL_NODES=str(1 << 28), MITHRIL_RECS=str(1 << 26))


def run(cmd, cap, env=None):
    t0 = time.time()
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=cap, env=env)
        return r.returncode, r.stdout, r.stderr, time.time() - t0
    except subprocess.TimeoutExpired:
        return -1, "", "TIMEOUT", time.time() - t0


def capped(cmd):
    return ["bash", "-c", f"ulimit -v {BUILD_MEM_KB}; exec \"$@\"", "--"] + cmd


def variant(name, subs, tmp, tag):
    src = os.path.join(tmp, f"{name}_{tag}.py")
    shutil.copy(os.path.join(PORTS, f"{name}.py"), src)
    for e in subs:
        subprocess.run(["sed", "-i", e, src], check=True)
    return src


def instructions(cmd, cap):
    rc, out, err, dt = run(["perf", "stat", "-x,", "-e", "instructions:u"] + cmd, cap, env=RUN_ENV)
    if rc != 0:
        return None, None
    m = re.search(r"^(\d+),,instructions:u", err, re.M)
    val = (out.strip().split("\n") or [""])[-1]
    return (int(m.group(1)) if m else None), val


def measure(name, tmp):
    r = {"name": name}
    subs, expect = SMALL[name]
    src = variant(name, subs, tmp, "small")
    # code metrics come from the unmodified port: at small sizes the
    # compile-time reducer may fold the whole program to a constant
    gen = os.path.join(tmp, f"{name}.gen.rs")
    rc, out, err, dt = run([DUMP, os.path.join(PORTS, f"{name}.py"), gen], BUILD_CAP_S)
    if rc == 0 and os.path.exists(gen):
        code = open(gen).read()
        r["gen_lines"] = code.count("\n")
        r["segments"] = len(re.findall(r"^fn sg_\d+\(", code, re.M))
        r["scalar"] = sorted(int(x) for x in re.findall(r"^fn s_(\d+)\(", code, re.M))
    else:
        r["error"] = "dump_gen: " + err.strip()[-160:]
        return r
    binp = os.path.join(tmp, f"{name}.small")
    rc, out, err, dt = run(capped([BIN, "build", src, "-o", binp]), BUILD_CAP_S)
    r["build_s"] = round(dt, 2)
    if rc != 0:
        r["error"] = "build: " + err.strip()[-160:]
        return r
    if name in KNOWN_SLOW:
        r["small_ok"] = None
        return r
    for t in (1, 16):
        rc, out, err, dt = run([binp, "--threads", str(t)], RUN_CAP_S, env=RUN_ENV)
        got = out.strip().split("\n")[-1] if out.strip() else err.strip()[-100:]
        r[f"small_t{t}"] = got
    r["small_ok"] = r["small_t1"] == expect and r["small_t16"] == expect
    if name in MID:
        msrc = variant(name, MID[name], tmp, "mid")
        mbin = os.path.join(tmp, f"{name}.mid")
        rc, out, err, dt = run(capped([BIN, "build", msrc, "-o", mbin]), BUILD_CAP_S)
        if rc == 0:
            ins, v1 = instructions([mbin, "--threads", "1"], RUN_CAP_S)
            rc2, out2, err2, _ = run([mbin, "--threads", "16"], RUN_CAP_S, env=RUN_ENV)
            v16 = out2.strip().split("\n")[-1] if out2.strip() else ""
            r["instr"] = ins
            r["mid_agree"] = (v1 is not None and v1 == v16)
            if "arena exhausted" in err2 or ins is None:
                r["mid_exhausted"] = True
    return r


def compare(cur, base):
    """Returns (hard failures, soft warnings) as lists of strings."""
    hard, soft = [], []
    if "error" in cur:
        return [cur["error"]], soft
    if cur.get("small_ok") is False:
        hard.append(f"checksum small t1={cur.get('small_t1')} t16={cur.get('small_t16')}")
    if cur.get("mid_exhausted"):
        hard.append("mid-size run exhausted the arena (memory regression)")
    elif "instr" in cur and cur.get("mid_agree") is False:
        hard.append("mid-size t1/t16 checksums differ")
    if not base:
        return hard, soft
    lost = sorted(set(base.get("scalar", [])) - set(cur.get("scalar", [])))
    if lost:
        hard.append(f"scalar lowering lost for fns {lost}")
    for k, tol in TOL.items():
        b, c = base.get(k), cur.get(k)
        if b and c and c > b * tol:
            msg = f"{k} {c} > {tol}x baseline {b}"
            (soft if k in SOFT else hard).append(msg)
    return hard, soft


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--update", action="store_true")
    ap.add_argument("--only", nargs="*")
    ap.add_argument("--jobs", type=int, default=8)
    a = ap.parse_args()
    for p in (BIN, DUMP):
        if not os.path.exists(p):
            sys.exit(f"missing {p}: cargo build --release -p mithril-cli --example dump_gen && cargo build --release -p mithril-cli")
    names = a.only or sorted(SMALL)
    base = json.load(open(BASELINE)) if os.path.exists(BASELINE) else {}
    tmp = tempfile.mkdtemp(prefix="mithril-ci-")
    t0 = time.time()
    with ThreadPoolExecutor(max_workers=a.jobs) as ex:
        results = list(ex.map(lambda n: measure(n, tmp), names))
    shutil.rmtree(tmp, ignore_errors=True)
    fails = 0
    print(f"{'bench':13} {'small':5} {'scalar':>6} {'segs':>5} {'lines':>6} {'build':>6} {'instr(G)':>9}  status")
    for r in results:
        b = base.get(r["name"], {})
        hard, soft = compare(r, b)
        fails += bool(hard)
        ins = r.get("instr")
        insg = f"{ins / 1e9:9.3f}" if ins else f"{'-':>9}"
        st = "OK" if not hard else "FAIL"
        notes = "; ".join(hard + [f"warn: {s}" for s in soft])
        small = {True: "PASS", False: "FAIL", None: "slow"}[r.get("small_ok")]
        print(f"{r['name']:13} {small:5} {len(r.get('scalar', [])):6} {r.get('segments', '-'):>5} {r.get('gen_lines', '-'):>6} {r.get('build_s', '-'):>6} {insg}  {st} {notes}")
    print(f"total {time.time() - t0:.1f}s, {fails} failing")
    if a.update:
        # merge: `--update --only X` refreshes X and keeps every other entry
        base = json.load(open(BASELINE)) if os.path.exists(BASELINE) else {}
        base.update({r["name"]: r for r in results})
        json.dump(base, open(BASELINE, "w"), indent=1, sort_keys=True)
        print(f"baseline written: {BASELINE}")
    sys.exit(fails)


if __name__ == "__main__":
    main()
