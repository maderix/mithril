#!/usr/bin/env python3
"""Starter programs: what someone writes in their first hour with Mithril.

Mithril's syntax is a subset of Python's, so each program here also runs
under CPython (with the array builtins defined as their value-semantics
equivalents). CPython's answer is the expected value; the reference
interpreter, 1 thread and 16 threads must print it. A program Mithril
rejects must be rejected with a message that names a line.

  tests/starter/run.py [--only NAME ...]

Exit status: number of programs that disagree, crash or fail to compile.
"""
import argparse
import glob
import math
import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
TARGET = os.environ.get("CARGO_TARGET_DIR", os.path.join(ROOT, "target"))
BIN = os.environ.get("MITHRIL_BIN", os.path.join(TARGET, "release", "mithril"))
CAP_S = 120
WANTS = {}


def array_set(a, i, v):
    b = list(a)
    b[i] = v
    return b


SHIM = {
    "array_new": lambda n, v: [v] * n,
    "array_get": lambda a, i: a[i],
    "array_set": array_set,
    "array_len": len,
    "sqrt": math.sqrt,
    "f32": float,
}


def show(v):
    """A CPython value as Mithril prints it: booleans are integers."""
    if isinstance(v, bool):
        return str(int(v))
    if isinstance(v, int):
        # Mithril's integers are 64-bit and wrap
        v &= (1 << 64) - 1
        return str(v - (1 << 64) if v >> 63 else v)
    if isinstance(v, tuple):
        return "(" + ", ".join(show(x) for x in v) + ")"
    return str(v)


def expected(path):
    """CPython's value, on a thread with room for deep recursion."""
    import threading
    out = []

    def go():
        env = dict(SHIM)
        exec(compile(open(path).read(), path, "exec"), env)
        out.append(show(env["main"]()))

    sys.setrecursionlimit(1 << 20)
    threading.stack_size(1 << 29)
    t = threading.Thread(target=go)
    t.start()
    t.join()
    return out[0] if out else "CPython failed"


def run(cmd):
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=CAP_S)
        return r.returncode, r.stdout.strip(), r.stderr.strip()
    except subprocess.TimeoutExpired:
        return -1, "", "TIMEOUT"


def check(path, tmp):
    name = os.path.basename(path)[:-3]
    want = WANTS[path]
    lanes = {}
    rc, out, err = run([BIN, "oracle", path])
    lanes["oracle"] = out if rc == 0 else None
    if rc != 0:
        return name, want, lanes, err
    exe = os.path.join(tmp, name)
    rc, _, err = run([BIN, "build", path, "-o", exe])
    if rc != 0:
        return name, want, lanes, "build: " + err
    for t in (1, 16):
        rc, out, err = run([exe, "--threads", str(t)])
        lanes[f"t{t}"] = out if rc == 0 else None
        if rc != 0:
            return name, want, lanes, f"t{t}: " + err
    return name, want, lanes, None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", nargs="*")
    a = ap.parse_args()
    paths = sorted(glob.glob(os.path.join(HERE, "*.py")))
    paths = [p for p in paths if os.path.basename(p) != "run.py"]
    if a.only:
        paths = [p for p in paths if os.path.basename(p)[:-3] in a.only]
    tmp = tempfile.mkdtemp(prefix="mithril-starter-")
    WANTS.update({p: expected(p) for p in paths})
    with ThreadPoolExecutor(max_workers=8) as ex:
        results = list(ex.map(lambda p: check(p, tmp), paths))
    fails = 0
    for name, want, lanes, err in results:
        got = set(lanes.values())
        ok = err is None and got == {want}
        fails += not ok
        if ok:
            print(f"{name:18} OK    {want}")
        elif err is not None:
            line = err.strip().split("\n")[0][:110]
            print(f"{name:18} ERROR want {want}: {line}")
        else:
            print(f"{name:18} WRONG want {want}: " + " ".join(f"{k}={v}" for k, v in lanes.items()))
    print(f"{len(results) - fails}/{len(results)} starter programs match CPython")
    sys.exit(fails)


if __name__ == "__main__":
    main()
