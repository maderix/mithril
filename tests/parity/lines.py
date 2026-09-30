#!/usr/bin/env python3
"""Line count of the whole execution system: crates/<c>/src/**/*.rs for the
eight crates below, plus crates/mithril-gpu/cuda/*.cu (plain `wc -l`: every
line, comments and blanks included). Prints per crate and the total.

Usage: tests/parity/lines.py [--max N]   (exit 1 when the total exceeds N)
"""
import argparse
import glob
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CRATES = ["mithril-front", "mithril-core", "mithril-net", "mithril-reassoc",
          "mithril-codegen", "mithril-cli", "mithril-rt", "mithril-gpu"]


def lines(path):
    with open(path, "rb") as f:
        return f.read().count(b"\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--max", type=int)
    a = ap.parse_args()
    total = 0
    for c in CRATES:
        files = glob.glob(os.path.join(ROOT, "crates", c, "src", "**", "*.rs"), recursive=True)
        if c == "mithril-gpu":
            files += glob.glob(os.path.join(ROOT, "crates", c, "cuda", "*.cu"))
        n = sum(lines(f) for f in files)
        total += n
        print(f"{c:18} {n:7}  ({len(files)} files)")
    print(f"{'total':18} {total:7}")
    if a.max is not None and total > a.max:
        print(f"over budget: {total} > {a.max}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
