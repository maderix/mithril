#!/usr/bin/env python3
"""Generality corpus: programs unlike the benchmark suite (sharing,
persistence, irregular recursion, dispatch, mixed ADT/array data).

For each program: the Mithril build must agree with the Python oracle at a
small size and with the idiomatic Rust twin (rust/<name>.rs) at full size;
then SEQ (1 thread) and PAR (16 threads) are timed against the Rust twin.

Usage: bench/general/run.py [--only NAME ...]
"""
import argparse, os, re, subprocess, sys, tempfile, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
BIN = os.path.join(ROOT, "target", "release", "mithril")
SHIM = os.path.join(ROOT, "bench", "ports", "_pyshim.py")
# small sizes for the oracle check (the Python shim runs these)
SMALL = {"collatz_mutual": 3000, "cow_versions": 3000, "dag_share": 3, "graph_dfs": 1,
         "interp": 5, "persist_map": 2000, "sorts": 300}


def timed(cmd, env=None):
    t = time.time()
    p = subprocess.run(cmd, capture_output=True, text=True, env=env)
    if p.returncode != 0:
        raise RuntimeError(f"{cmd[0]} failed: {p.stderr[-300:]}")
    return p.stdout.strip().splitlines()[-1], time.time() - t


def oracle(src):
    code = f"""
import sys, threading, runpy
sys.setrecursionlimit(10**7)
threading.stack_size(1 << 28)
def go():
    sys.argv = ['_pyshim.py', {src!r}]
    runpy.run_path({SHIM!r}, run_name='__main__')
t = threading.Thread(target=go); t.start(); t.join()
"""
    p = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True)
    return p.stdout.strip().splitlines()[-1]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", nargs="*")
    a = ap.parse_args()
    names = sorted(f[:-3] for f in os.listdir(HERE) if f.endswith(".py") and f != "run.py")
    if a.only:
        names = [n for n in names if n in a.only]
    tmp = tempfile.mkdtemp(prefix="mithril-general-")
    print(f"{'program':16} {'oracle':6} {'twin':5} {'rust':>7} {'t1':>7} {'t16':>7} {'t1/rust':>8} {'t16/rust':>9}")
    for n in names:
        src = os.path.join(HERE, n + ".py")
        text = open(src).read()
        small = re.sub(r"return run\((\d+)\)", f"return run({SMALL[n]})", text)
        sp = os.path.join(tmp, n + "_small.py")
        open(sp, "w").write(small)
        sb = os.path.join(tmp, n + "_small")
        subprocess.run([BIN, "build", sp, "-o", sb], check=True, capture_output=True)
        ok = timed([sb, "--threads", "4"])[0] == oracle(sp)
        big = os.path.join(tmp, n)
        subprocess.run([BIN, "build", src, "-o", big], check=True, capture_output=True)
        rsrc = os.path.join(HERE, "rust", n + ".rs")
        rbin = os.path.join(tmp, n + "_rs")
        subprocess.run(["rustc", "-O", rsrc, "-o", rbin], check=True, capture_output=True)
        rout, rt = timed([rbin])
        o1, t1 = timed([big, "--threads", "1"])
        o16, t16 = timed([big, "--threads", "16"])
        twin = rout == o1 == o16
        print(f"{n:16} {'ok' if ok else 'FAIL':6} {'ok' if twin else 'DIFF':5} {rt:7.2f} {t1:7.2f} {t16:7.2f} {t1 / rt:8.2f} {t16 / rt:9.2f}")


if __name__ == "__main__":
    main()
