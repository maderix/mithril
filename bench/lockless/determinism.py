#!/usr/bin/env python3
"""Run each implementation N times per thread count; count distinct outputs.

    python3 bench/lockless/determinism.py [--runs 100] [--threads 1,4,16]
        [--size small|big] [--impl mithril,c,rust] [--programs bfs,fsum]

Builds each program once per implementation, then runs it --runs times at
each thread count and compares the whole stdout byte for byte:
  mithril  target/release/mithril build main.py -o <out>; <out> --threads T
  c        gcc -O2 -fopenmp -pthread main.c -o <out>; OMP_NUM_THREADS=T <out>
  rust     cargo build --release (package bench/lockless/rust);
           RAYON_NUM_THREADS=T <out>
At --size small the sources are copied to bench/lockless/.build/ and the
SIZE line rewritten with the sed expression in small.json; at big they are
built as they are. Reports, per program, implementation and thread count,
the number of distinct outputs, and whether the checksum line matches the
expected one (small.json or expected.txt; fsum's expected value is
Mithril's). Only CPU lanes: this script never passes --gpu.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
PROGRAMS = ["histogram", "hashbuild", "bfs", "components", "fsum"]
IMPLS = ["mithril", "c", "rust"]


def sed(expr, src, dst):
    dst.parent.mkdir(parents=True, exist_ok=True)
    with open(dst, "w") as f:
        subprocess.run(["sed", expr, str(src)], stdout=f, check=True)


def expected(size):
    if size == "small":
        return {k: v["expected"] for k, v in json.loads((HERE / "small.json").read_text()).items() if not k.startswith("_")}
    out = {}
    for line in (HERE / "expected.txt").read_text().splitlines():
        if line.strip() and not line.startswith("#"):
            name, value = line.split(" ", 1)
            out[name] = value.strip()
    return out


def build(impl, name, size, mithril, small):
    """Build one program; return the command prefix to run it and the env var
    (or flag) that sets its thread count."""
    work = HERE / ".build" / size
    work.mkdir(parents=True, exist_ok=True)
    if impl == "mithril":
        src = HERE / name / "main.py"
        if size == "small":
            sed(small[name]["sed"], src, work / name / "main.py")
            src = work / name / "main.py"
        out = work / f"{name}.mithril"
        subprocess.run([str(mithril), "build", str(src), "-o", str(out)], check=True, stdout=subprocess.DEVNULL)
        return [str(out)], ("flag", "--threads")
    if impl == "c":
        src = HERE / name / "main.c"
        if size == "small":
            sed(small[name]["sed"], src, work / name / "main.c")
            src = work / name / "main.c"
        out = work / f"{name}.c.bin"
        subprocess.run(["gcc", "-O2", "-fopenmp", "-pthread", str(src), "-o", str(out)], check=True)
        return [str(out)], ("env", "OMP_NUM_THREADS")
    pkg = HERE / "rust"
    if size == "small":
        dst = work / "rust"
        for rel in ["Cargo.toml", "Cargo.lock", "src/lib.rs"]:
            (dst / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(pkg / rel, dst / rel)
        for n in PROGRAMS:
            sed(small[n]["sed"], pkg / "src" / "bin" / f"{n}.rs", dst / "src" / "bin" / f"{n}.rs")
        pkg = dst
    cmd = ["cargo", "build", "--release", "--quiet", "--bin", name]
    if subprocess.run(cmd, cwd=pkg).returncode != 0:
        subprocess.run(cmd + ["--offline"], cwd=pkg, check=True)
    return [str(pkg / "target" / "release" / name)], ("env", "RAYON_NUM_THREADS")


def run(cmd, how, threads, timeout):
    env = dict(os.environ)
    if how[0] == "flag":
        cmd = cmd + [how[1], str(threads)]
    else:
        env[how[1]] = str(threads)
    r = subprocess.run(cmd, env=env, capture_output=True, timeout=timeout)
    return r.stdout + (b"" if r.returncode == 0 else b"<exit %d>" % r.returncode)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--runs", type=int, default=100)
    ap.add_argument("--threads", default="1,4,16")
    ap.add_argument("--size", choices=["small", "big"], default="small")
    ap.add_argument("--impl", default=",".join(IMPLS))
    ap.add_argument("--programs", default=",".join(PROGRAMS))
    ap.add_argument("--timeout", type=float, default=300)
    ap.add_argument("--mithril", default=str(ROOT / "target" / "release" / "mithril"))
    a = ap.parse_args()
    small = {k: v for k, v in json.loads((HERE / "small.json").read_text()).items() if not k.startswith("_")}
    want = expected(a.size)
    threads = [int(t) for t in a.threads.split(",")]
    print(f"size {a.size}, {a.runs} runs per thread count, threads {threads}")
    print()
    print("| program | impl | " + " | ".join(f"t{t}" for t in threads) + " | all | checksum | expected |")
    print("|---|---|" + "---|" * len(threads) + "---|---|---|")
    bad = 0
    for name in a.programs.split(","):
        for impl in a.impl.split(","):
            cmd, how = build(impl, name, a.size, a.mithril, small)
            every = set()
            cells = []
            for t in threads:
                seen = set(run(cmd, how, t, a.timeout) for _ in range(a.runs))
                every |= seen
                cells.append(str(len(seen)))
            lines = sorted({o.decode(errors="replace").strip().splitlines()[-1] if o.strip() else "<empty>" for o in every})
            shown = lines[0] if len(lines) == 1 else f"{len(lines)} values, e.g. {lines[0]}"
            fsum_other = name == "fsum" and impl != "mithril"
            ok = "n/a (own bits)" if fsum_other else ("yes" if lines == [want[name]] else "NO")
            if ok == "NO" or (impl == "mithril" and len(every) != 1):
                bad += 1
            print(f"| {name} | {impl} | " + " | ".join(cells) + f" | {len(every)} | {shown} | {ok} |")
            sys.stdout.flush()
    print()
    print("cells: distinct outputs over the runs at that thread count; all: over every run.")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
