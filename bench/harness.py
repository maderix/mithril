#!/usr/bin/env python3
"""Mithril benchmark harness.

For every benchmark ported under bench/ports/<name>.py this runs:

  * the C twin  (bench/ports/<name>.c, compiled with `gcc -O2`, if present)
  * `mithril run bench/ports/<name>.py --threads 1`   (SEQ)
  * `mithril run bench/ports/<name>.py --threads 16`  (PAR16)
  * `mithril run bench/ports/<name>.py --threads 16 --gpu`
    (GPU, only when the environment sets MITHRIL_GPU=1)

Each lane is executed N=3 times and the minimum wall time is kept.
EVERY run's stdout must equal the checksum recorded in bench/expected.txt;
any mismatch marks the row FAILED and the harness exits nonzero at the end.

Results land in bench/results.md as a table

  name | C | SEQ | PAR16 | GPU | SEQ/C | PAR/C | GPU/C

with optional reference columns (B2-SEQ | B2-PAR | B2-GPU) imported from
bench/reference.csv (columns: name,seq,par,gpu) when that file exists.

The mithril binary is built once at harness start via
`cargo build --release -p mithril-cli` and used from target/release/mithril.

Usage:
  python3 bench/harness.py [--dry-run] [-n RUNS] [--timeout SECS]
                           [--only NAME [NAME ...]]

  --dry-run   lists what would run and validates expected.txt coverage
              without building or executing anything.
"""

import argparse
import csv
import os
import shutil
import subprocess
import sys
import tempfile
import time

BENCH_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(BENCH_DIR)
PORTS_DIR = os.path.join(BENCH_DIR, "ports")
EXPECTED_TXT = os.path.join(BENCH_DIR, "expected.txt")
reference_CSV = os.path.join(BENCH_DIR, "reference.csv")
RESULTS_MD = os.path.join(BENCH_DIR, "results.md")
MITHRIL_BIN = os.path.join(REPO_ROOT, "target", "release", "mithril")

N_RUNS_DEFAULT = 3
PAR_THREADS = 16


def load_expected(path):
    """Parse expected.txt: `<name> <checksum>` per line, `#` comments allowed."""
    expected = {}
    if not os.path.isfile(path):
        return expected
    with open(path) as fh:
        for lineno, raw in enumerate(fh, 1):
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split()
            if len(parts) != 2:
                sys.exit("expected.txt:%d: malformed line: %r" % (lineno, raw))
            name, checksum = parts
            if name in expected:
                sys.exit("expected.txt:%d: duplicate entry for %s" % (lineno, name))
            expected[name] = checksum
    return expected


def discover():
    """Return (ports, c_twins): sorted benchmark names found under bench/ports/."""
    ports, c_twins = [], []
    if os.path.isdir(PORTS_DIR):
        for entry in sorted(os.listdir(PORTS_DIR)):
            stem, ext = os.path.splitext(entry)
            if stem.startswith("_"):
                continue
            if ext == ".py":
                ports.append(stem)
            elif ext == ".c":
                c_twins.append(stem)
    return ports, c_twins


def load_reference(path):
    """Parse optional reference.csv (name,seq,par,gpu). Returns {name: (seq,par,gpu)}."""
    table = {}
    if not os.path.isfile(path):
        return table
    with open(path, newline="") as fh:
        for row in csv.reader(fh):
            if not row or row[0].strip().startswith("#"):
                continue
            cells = [c.strip() for c in row]
            if cells[0].lower() == "name":  # header row
                continue
            name = cells[0]
            vals = []
            for cell in cells[1:4] + [""] * (4 - len(cells)):
                try:
                    vals.append(float(cell))
                except ValueError:
                    vals.append(None)
            table[name] = tuple(vals[:3])
    return table


def run_once(cmd, timeout, env=None):
    """Run cmd once; return (wall_seconds, stdout_stripped, ok_exit)."""
    t0 = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd, capture_output=True, text=True, timeout=timeout,
            cwd=REPO_ROOT, env=env,
        )
    except subprocess.TimeoutExpired:
        return time.perf_counter() - t0, "<timeout after %gs>" % timeout, False
    wall = time.perf_counter() - t0
    return wall, proc.stdout.strip(), proc.returncode == 0


def run_lane(label, cmd, expected, n_runs, timeout, env=None):
    """Run one lane N times; return (min_wall or None, list of error strings)."""
    best, errors = None, []
    for i in range(n_runs):
        wall, out, ok = run_once(cmd, timeout, env=env)
        if not ok:
            errors.append("%s run %d/%d: nonzero exit or timeout (stdout=%r)"
                          % (label, i + 1, n_runs, out[:200]))
            return None, errors
        if out != expected:
            errors.append("%s run %d/%d: checksum mismatch: got %r, want %r"
                          % (label, i + 1, n_runs, out[:200], expected))
            return None, errors
        if best is None or wall < best:
            best = wall
    return best, errors


def fmt_time(secs):
    return "%.3fs" % secs if secs is not None else "-"


def fmt_ratio(num, den):
    if num is None or den is None or den == 0:
        return "-"
    return "%.2fx" % (num / den)


def build_mithril():
    print("[harness] building mithril: cargo build --release -p mithril-cli")
    proc = subprocess.run(
        ["cargo", "build", "--release", "-p", "mithril-cli"],
        cwd=REPO_ROOT,
    )
    if proc.returncode != 0:
        sys.exit("[harness] cargo build failed (exit %d)" % proc.returncode)
    if not os.path.isfile(MITHRIL_BIN):
        sys.exit("[harness] mithril binary not found at %s" % MITHRIL_BIN)


def compile_c(name, out_dir):
    """gcc -O2 the C twin; return binary path or None on failure."""
    src = os.path.join(PORTS_DIR, name + ".c")
    binary = os.path.join(out_dir, name)
    proc = subprocess.run(
        ["gcc", "-O2", "-o", binary, src, "-lm"],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        print("[harness] gcc failed for %s:\n%s" % (name, proc.stderr), file=sys.stderr)
        return None
    return binary


def dry_run(ports, c_twins, expected, gpu_enabled, n_runs):
    print("[dry-run] bench dir : %s" % BENCH_DIR)
    print("[dry-run] mithril   : %s (built via `cargo build --release -p mithril-cli`)"
          % MITHRIL_BIN)
    print("[dry-run] runs/lane : %d (min wall-time kept)" % n_runs)
    print("[dry-run] GPU lane  : %s (MITHRIL_GPU=%s)"
          % ("ENABLED" if gpu_enabled else "disabled",
             os.environ.get("MITHRIL_GPU", "<unset>")))
    print()

    problems = []
    if not expected:
        problems.append("expected.txt is missing or empty")

    if not ports:
        print("[dry-run] no ports found in bench/ports/*.py yet (Task 11 lands them);")
        print("[dry-run] nothing would run. Coverage of already-present artifacts:")
    for name in ports:
        exp = expected.get(name)
        if exp is None:
            problems.append("port %s.py has no entry in expected.txt" % name)
        print("[dry-run] %-14s expected=%s" % (name, exp or "MISSING"))
        if name in c_twins:
            print("            C     : gcc -O2 ports/%s.c && ./%s   x%d" % (name, name, n_runs))
        else:
            print("            C     : (no %s.c — column will be '-')" % name)
        print("            SEQ   : %s run bench/ports/%s.py --threads 1   x%d"
              % (MITHRIL_BIN, name, n_runs))
        print("            PAR16 : %s run bench/ports/%s.py --threads %d   x%d"
              % (MITHRIL_BIN, name, PAR_THREADS, n_runs))
        if gpu_enabled:
            print("            GPU   : %s run bench/ports/%s.py --threads %d --gpu   x%d"
                  % (MITHRIL_BIN, name, PAR_THREADS, n_runs))

    # Coverage report over everything present, ports or not.
    for name in c_twins:
        if name not in expected:
            problems.append("C twin %s.c has no entry in expected.txt" % name)
    pending = sorted(set(expected) - set(ports))
    covered_c = sorted(set(expected) & set(c_twins))
    print()
    print("[dry-run] expected.txt entries : %d" % len(expected))
    print("[dry-run] ports (*.py)         : %d" % len(ports))
    print("[dry-run] C twins (*.c)        : %d (%d with expected checksums)"
          % (len(c_twins), len(covered_c)))
    if pending:
        print("[dry-run] expected entries awaiting a .py port: %s" % " ".join(pending))
    if os.path.isfile(reference_CSV):
        print("[dry-run] reference.csv present — reference columns would be imported")
    else:
        print("[dry-run] reference.csv absent — reference columns omitted")

    if problems:
        print()
        for p in problems:
            print("[dry-run] ERROR: %s" % p)
        return 1
    print("[dry-run] coverage OK")
    return 0


def main():
    ap = argparse.ArgumentParser(description="Mithril benchmark harness")
    ap.add_argument("--dry-run", action="store_true",
                    help="list what would run and validate expected.txt coverage")
    ap.add_argument("-n", "--runs", type=int, default=N_RUNS_DEFAULT,
                    help="runs per lane, min wall-time kept (default %d)" % N_RUNS_DEFAULT)
    ap.add_argument("--timeout", type=float, default=900.0,
                    help="per-run timeout in seconds (default 900)")
    ap.add_argument("--only", nargs="+", metavar="NAME",
                    help="restrict to these benchmark names")
    args = ap.parse_args()

    expected = load_expected(EXPECTED_TXT)
    ports, c_twins = discover()
    if args.only:
        unknown = sorted(set(args.only) - set(ports))
        if unknown and not args.dry_run:
            sys.exit("--only names have no port: %s" % " ".join(unknown))
        ports = [p for p in ports if p in set(args.only)]
    gpu_enabled = os.environ.get("MITHRIL_GPU") == "1"

    if args.dry_run:
        sys.exit(dry_run(ports, c_twins, expected, gpu_enabled, args.runs))

    if not ports:
        sys.exit("[harness] no benchmarks found under %s (need <name>.py)" % PORTS_DIR)
    missing = [p for p in ports if p not in expected]
    if missing:
        sys.exit("[harness] expected.txt is missing entries for: %s" % " ".join(missing))

    build_mithril()
    reference = load_reference(reference_CSV)
    have_reference = bool(reference)

    rows = []          # (name, c, seq, par, gpu, failed, errors)
    any_failed = False
    tmpdir = tempfile.mkdtemp(prefix="mithril-bench-")
    try:
        for name in ports:
            exp = expected[name]
            port = os.path.join(PORTS_DIR, name + ".py")
            errors = []
            print("[harness] === %s (expect %s) ===" % (name, exp))

            c_time = None
            if name in c_twins:
                binary = compile_c(name, tmpdir)
                if binary is None:
                    errors.append("C: gcc -O2 failed to compile")
                else:
                    c_time, errs = run_lane("C", [binary], exp, args.runs, args.timeout)
                    errors += errs
            print("[harness]   C     : %s" % fmt_time(c_time))

            seq_time, errs = run_lane(
                "SEQ", [MITHRIL_BIN, "run", port, "--threads", "1"],
                exp, args.runs, args.timeout)
            errors += errs
            print("[harness]   SEQ   : %s" % fmt_time(seq_time))

            par_time, errs = run_lane(
                "PAR16", [MITHRIL_BIN, "run", port, "--threads", str(PAR_THREADS)],
                exp, args.runs, args.timeout)
            errors += errs
            print("[harness]   PAR16 : %s" % fmt_time(par_time))

            gpu_time = None
            if gpu_enabled:
                gpu_time, errs = run_lane(
                    "GPU",
                    [MITHRIL_BIN, "run", port, "--threads", str(PAR_THREADS), "--gpu"],
                    exp, args.runs, args.timeout)
                errors += errs
                print("[harness]   GPU   : %s" % fmt_time(gpu_time))

            failed = bool(errors)
            if failed:
                any_failed = True
                for e in errors:
                    print("[harness]   FAILED %s: %s" % (name, e), file=sys.stderr)
            rows.append((name, c_time, seq_time, par_time, gpu_time, failed, errors))
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)

    write_results(rows, reference, have_reference, gpu_enabled, args.runs)
    print("[harness] wrote %s" % RESULTS_MD)
    if any_failed:
        print("[harness] FAILURES present — see results.md", file=sys.stderr)
        sys.exit(1)


def write_results(rows, reference, have_reference, gpu_enabled, n_runs):
    header = ["name", "C", "SEQ", "PAR16", "GPU", "SEQ/C", "PAR/C", "GPU/C"]
    if have_reference:
        header += ["B2-SEQ", "B2-PAR", "B2-GPU"]
    lines = [
        "# Mithril benchmark results",
        "",
        "Wall-clock minimum of %d runs per lane. `SEQ` = `mithril run --threads 1`, "
        "`PAR16` = `--threads %d`, `GPU` = `--gpu`%s. Ratios are mithril-time / C-time "
        "(lower is better). Every run's stdout was checked against `expected.txt`; "
        "a FAILED row means at least one lane printed the wrong checksum or crashed."
        % (n_runs, PAR_THREADS,
           "" if gpu_enabled else " (disabled: MITHRIL_GPU!=1)"),
        "",
        "| " + " | ".join(header) + " |",
        "|" + "|".join(["---"] * len(header)) + "|",
    ]
    for name, c, seq, par, gpu, failed, errors in rows:
        cells = [
            name + (" **FAILED**" if failed else ""),
            fmt_time(c), fmt_time(seq), fmt_time(par), fmt_time(gpu),
            fmt_ratio(seq, c), fmt_ratio(par, c), fmt_ratio(gpu, c),
        ]
        if have_reference:
            b2 = reference.get(name, (None, None, None))
            cells += [fmt_time(v) for v in b2]
        lines.append("| " + " | ".join(cells) + " |")
    failed_rows = [(n, errs) for n, _, _, _, _, f, errs in rows if f]
    if failed_rows:
        lines += ["", "## Failures", ""]
        for name, errs in failed_rows:
            for e in errs:
                lines.append("- `%s`: %s" % (name, e))
    lines.append("")
    with open(RESULTS_MD, "w") as fh:
        fh.write("\n".join(lines))


if __name__ == "__main__":
    main()
