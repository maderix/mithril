#!/usr/bin/env python3
"""Run each implementation N times per thread count; count distinct outputs.

    python3 bench/lockless/determinism.py [--runs 100] [--threads 1,4,16]
        [--size small|big] [--impl mithril,c,rust] [--programs bfs,fsum]

Implementations of a program (variants are found by file name):
  mithril        <name>/main.py
  c, c_<v>       <name>/main.c, <name>/main_<v>.c (e.g. c_fold, c_tree)
  rust, rust_<v> rust/src/bin/<name>.rs, rust/src/bin/<name>_<v>.rs
--impl selects by base (c selects c and every c_<v>) or by full label.
Builds each once, then runs it --runs times at each thread count and
compares the whole stdout byte for byte:
  mithril  target/release/mithril build main.py -o <out>; <out> --threads T
  c        gcc -O2 -fopenmp -pthread main*.c -o <out>; OMP_NUM_THREADS=T <out>
  rust     cargo build --release (package bench/lockless/rust);
           RAYON_NUM_THREADS=T <out>
At --size small the sources are copied to bench/lockless/.build/ and the
SIZE line rewritten with the sed expression in small.json; at big they are
built as they are. Reports, per program, implementation and thread count,
the number of distinct outputs, and whether the checksum line matches the
expected one (small.json or expected.txt). fsum's expected value is the
fixed-shape tree sum's (Mithril, c_tree, rust_tree); its c and rust
variants use their idiomatic reductions and print their own bits (OWN_BITS).
A run that times out or a build that fails is a failure of that row; the
script goes on with the next. Exit status 1 if any row failed. Only CPU
lanes: this script never passes --gpu.
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
# variants whose schedule-dependent bits are expected (idiomatic reductions)
OWN_BITS = {"fsum": {"c", "rust"}}


def variants(name):
    """Labels and sources of every implementation of a program."""
    out = [("mithril", HERE / name / "main.py")]
    for f in sorted((HERE / name).glob("main*.c")):
        out.append(("c" + f.stem[len("main"):], f))
    for f in sorted((HERE / "rust" / "src" / "bin").glob(f"{name}*.rs")):
        if f.stem == name or f.stem.startswith(name + "_"):
            out.append(("rust" + f.stem[len(name):], f))
    return out


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


def build(label, src, name, size, mithril, small):
    """Build one implementation; return the command to run it and the env
    var (or flag) that sets its thread count."""
    work = HERE / ".build" / size
    work.mkdir(parents=True, exist_ok=True)
    impl = label.split("_")[0]
    if impl == "mithril":
        if size == "small":
            sed(small[name]["sed"], src, work / name / src.name)
            src = work / name / src.name
        out = work / f"{name}.mithril"
        subprocess.run([str(mithril), "build", str(src), "-o", str(out)], check=True, stdout=subprocess.DEVNULL)
        return [str(out)], ("flag", "--threads")
    if impl == "c":
        if size == "small":
            sed(small[name]["sed"], src, work / name / src.name)
            src = work / name / src.name
        out = work / f"{name}.{label}.bin"
        subprocess.run(["gcc", "-O2", "-fopenmp", "-pthread", str(src), "-o", str(out)], check=True)
        return [str(out)], ("env", "OMP_NUM_THREADS")
    pkg = HERE / "rust"
    if size == "small":
        dst = work / "rust"
        for rel in ["Cargo.toml", "Cargo.lock", "src/lib.rs"]:
            (dst / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(pkg / rel, dst / rel)
        for n in PROGRAMS:
            for _, f in variants(n):
                if f.suffix == ".rs":
                    sed(small[n]["sed"], f, dst / "src" / "bin" / f.name)
        pkg = dst
    cmd = ["cargo", "build", "--release", "--quiet", "--bin", src.stem]
    if subprocess.run(cmd, cwd=pkg).returncode != 0:
        subprocess.run(cmd + ["--offline"], cwd=pkg, check=True)
    return [str(pkg / "target" / "release" / src.stem)], ("env", "RAYON_NUM_THREADS")


def run(cmd, how, threads, timeout):
    env = dict(os.environ)
    if how[0] == "flag":
        cmd = cmd + [how[1], str(threads)]
    else:
        env[how[1]] = str(threads)
    try:
        r = subprocess.run(cmd, env=env, capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return b"<timeout>"
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
    sel = set(a.impl.split(","))
    for name in a.programs.split(","):
        for label, src in variants(name):
            if label not in sel and label.split("_")[0] not in sel:
                continue
            try:
                cmd, how = build(label, src, name, a.size, a.mithril, small)
            except subprocess.CalledProcessError as e:
                bad += 1
                print(f"| {name} | {label} | " + " | ".join("-" for _ in threads) + f" | - | build failed ({e.returncode}) | NO |")
                continue
            every = set()
            cells = []
            timeouts = 0
            for t in threads:
                outs = [run(cmd, how, t, a.timeout) for _ in range(a.runs)]
                n_to = sum(o == b"<timeout>" for o in outs)
                timeouts += n_to
                seen = set(outs)
                every |= seen
                cells.append(str(len(seen)) + (f" ({n_to} timeouts)" if n_to else ""))
            lines = sorted({o.decode(errors="replace").strip().splitlines()[-1] if o.strip() else "<empty>" for o in every})
            shown = lines[0] if len(lines) == 1 else f"{len(lines)} values, e.g. {lines[0]}"
            own = label in OWN_BITS.get(name, ())
            ok = "n/a (own bits)" if own else ("yes" if lines == [want[name]] else "NO")
            if timeouts or ok == "NO" or (not own and len(every) != 1):
                bad += 1
                if ok != "NO":
                    ok += ", FAILED"
            print(f"| {name} | {label} | " + " | ".join(cells) + f" | {len(every)} | {shown} | {ok} |")
            sys.stdout.flush()
    print()
    print("cells: distinct outputs over the runs at that thread count; all: over every run.")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
