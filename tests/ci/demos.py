#!/usr/bin/env python3
"""Demo suite: every demo and every example program of the field guide, at
a small size, on the reference interpreter, 1 thread, 16 threads and (when
there is one) the GPU. All lanes must print the same value.

  tests/ci/demos.py [--only NAME ...] [--no-gpu]

Exit status: number of programs whose lanes disagree or fail.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
TARGET = os.environ.get("CARGO_TARGET_DIR", os.path.join(ROOT, "target"))
BIN = os.environ.get("MITHRIL_BIN", os.path.join(TARGET, "release", "mithril"))
CAP_S = 600

# name: (source, {constant function: small value}, {text: small text})
PROGRAMS = {
    "black_hole": ("demos/black_hole.py", {"width": 16, "height": 10, "frames": 2, "first_frame": 300}, {}),
    "sphere_field": ("demos/sphere_field.py", {"width": 24, "height": 16, "spp": 2, "depth": 3, "rows": 3}, {}),
    "cornell_whitted": ("demos/cornell_whitted.py", {"size": 12}, {}),
    "cornell_path": ("demos/cornell_path.py", {"size": 8, "spp": 4}, {}),
    "collatz": ("docs/examples/collatz.py", {}, {"array_new(300000, 0)": "array_new(3000, 0)"}),
    "msort": ("docs/examples/msort.py", {}, {"array_new(65536, 0)": "array_new(2000, 0)"}),
    "queens": ("docs/examples/queens.py", {}, {"array_new(10, 0)": "array_new(7, 0)"}),
    "specialize": ("docs/examples/specialize.py", {}, {}),
}


def small_source(name, tmp):
    path, consts, texts = PROGRAMS[name]
    src = open(os.path.join(ROOT, path)).read()
    for f, v in consts.items():
        src, n = re.subn(rf"(def {f}\(\):\n    return ).*", rf"\g<1>{v}", src)
        assert n == 1, f"{name}: {f}() not found"
    for old, new in texts.items():
        assert old in src, f"{name}: {old!r} not found"
        src = src.replace(old, new)
    out = os.path.join(tmp, f"{name}.py")
    open(out, "w").write(src)
    return out


def run(cmd):
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=CAP_S)
        return r.returncode, r.stdout.strip(), r.stderr.strip()
    except subprocess.TimeoutExpired:
        return -1, "", "TIMEOUT"


def check(name, tmp, gpu):
    t0 = time.time()
    src = small_source(name, tmp)
    lanes = {}
    rc, out, err = run([BIN, "oracle", src])
    lanes["oracle"] = out if rc == 0 else f"ERROR {err[-120:]}"
    rc, _, err = run([BIN, "build", src, "-o", src + ".cpu"])
    for t in (1, 16):
        if rc != 0:
            lanes[f"t{t}"] = f"BUILD ERROR {err[-120:]}"
            continue
        r2, out, err2 = run([src + ".cpu", "--threads", str(t)])
        lanes[f"t{t}"] = out if r2 == 0 else f"ERROR {err2[-120:]}"
    if gpu:
        rc, _, err = run([BIN, "build", "--gpu", src, "-o", src + ".gpu"])
        r2, out, err2 = run([BIN, "exec", src + ".gpu"]) if rc == 0 else (rc, "", err)
        lanes["gpu"] = out if r2 == 0 else f"ERROR {err2[-120:]}"
    return name, lanes, time.time() - t0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", nargs="*")
    ap.add_argument("--no-gpu", action="store_true")
    a = ap.parse_args()
    if not os.path.exists(BIN):
        sys.exit(f"missing {BIN}: cargo build --release -p mithril-cli")
    gpu = not a.no_gpu and shutil.which("nvidia-smi") is not None
    tmp = tempfile.mkdtemp(prefix="mithril-demos-")
    names = a.only or list(PROGRAMS)
    with ThreadPoolExecutor(max_workers=4) as ex:
        results = list(ex.map(lambda n: check(n, tmp, gpu), names))
    shutil.rmtree(tmp, ignore_errors=True)
    fails = 0
    for name, lanes, dt in results:
        ok = len(set(lanes.values())) == 1 and not any(v.startswith(("ERROR", "BUILD ERROR")) for v in lanes.values())
        fails += not ok
        value = lanes["oracle"] if ok else " | ".join(f"{k}={v[:60]}" for k, v in lanes.items())
        print(f"{name:16} {'OK' if ok else 'FAIL':4} {dt:6.1f}s  {'/'.join(lanes)}  {value[:100]}")
    print(f"{len(results) - fails}/{len(results)} programs agree on every lane")
    sys.exit(fails)


if __name__ == "__main__":
    main()
