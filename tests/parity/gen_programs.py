#!/usr/bin/env python3
"""Export the random-program generator of
crates/mithril-net/tests/schedule_test.rs (struct Gen, fn program) as files:
tests/parity/gen/g<p>.py for p = 1..N (default 500), byte-identical to the
Rust generator's program(p) (checked with --check against a rustc build of
the Rust source's generator).

Usage: tests/parity/gen_programs.py [N] [--check]
"""
import os
import re
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
M = (1 << 64) - 1

PRELUDE = (
    "@data\nclass L:\n    Nil: ()\n    Cons: (h, t)\n\n\n"
    "def ap(f, v):\n    return f(v)\n\n\n"
    "def pair(a, b):\n    return (a, b)\n\n\n"
    "def build(n, s):\n    if n == 0:\n        return Nil()\n    return Cons(s + n, build(n - 1, s))\n\n\n"
    "def total(l):\n    match l:\n        case Nil():\n            return 0\n        case Cons(h, t):\n            return h + total(t)\n\n\n"
)


class Gen:
    def __init__(self, p):
        self.s = ((p * 0x9E3779B97F4A7C15) & M) | 1

    def pick(self, n):
        s = self.s
        s ^= (s << 13) & M
        s ^= s >> 7
        s ^= (s << 17) & M
        self.s = s
        return s % n

    def expr(self, vs, helpers, d):
        k = self.pick(2) if d == 0 else self.pick(8)
        if k == 0:
            return str(self.pick(20))
        if k == 1:
            if vs:
                return vs[self.pick(len(vs))]
            return str(self.pick(20))
        if k == 2:
            op = ["+", "-", "*", "&", "^"][self.pick(5)]
            a = self.expr(vs, helpers, d - 1)
            b = self.expr(vs, helpers, d - 1)
            return f"({a} {op} {b})"
        if k == 3:
            a, b, c, e = (self.expr(vs, helpers, d - 1) for _ in range(4))
            return f"({a} if {b} < {c} else {e})"
        if k == 4:
            if helpers > 0:
                h = self.pick(helpers)
                a = self.expr(vs, helpers, d - 1)
                b = self.expr(vs, helpers, d - 1)
                return f"h{h}({a}, {b})"
            return str(self.pick(20))
        if k == 5:
            a = self.expr(vs, helpers, d - 1)
            c = self.pick(9)
            b = self.expr(vs, helpers, d - 1)
            return f"ap(lambda x: x * {a} + {c}, {b})"
        if k == 6:
            n = self.pick(7)
            a = self.expr(vs, helpers, d - 1)
            return f"total(build({n}, {a}))"
        a = self.expr(vs, helpers, d - 1)
        b = self.expr(vs, helpers, d - 1)
        i = self.pick(2)
        return f"pair({a}, {b})[{i}]"

    def program(self):
        s = PRELUDE
        for i in range(3):
            x = self.expr(["a", "b"], i, 3)
            r = self.expr(["a", "b", "x"], i, 3)
            s += f"def h{i}(a, b):\n    x = {x}\n    return {r}\n\n\n"
        y = self.expr([], 3, 3)
        k = self.expr(["y"], 3, 2)
        n = self.pick(7)
        r1 = self.expr(["y"], 3, 3)
        r2 = self.expr(["y"], 3, 3)
        s += (f"def main():\n    y = {y}\n    f = lambda x: x * {k} + y\n    l = build({n}, y)\n"
              f"    return ({r1}, {r2}, f({r1}) + f(y), total(l) + total(l))\n")
        return s


def rust_programs(n):
    """program(1..=n) from the Rust generator itself (Gen + program copied
    out of schedule_test.rs, compiled standalone)."""
    src = open(os.path.join(ROOT, "crates/mithril-net/tests/schedule_test.rs")).read()
    body = src[src.index("fn xorshift"):src.index("fn module(")]
    main = f'fn main() {{ for p in 1..={n}u64 {{ print!("{{}}\\x00", program(p)); }} }}\n'
    d = tempfile.mkdtemp()
    open(os.path.join(d, "g.rs"), "w").write(body + main)
    subprocess.run(["rustc", "-O", "g.rs", "-o", "g"], cwd=d, check=True)
    out = subprocess.run([os.path.join(d, "g")], capture_output=True, text=True, check=True).stdout
    return out.split("\x00")[:n]


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    n = int(args[0]) if args else 500
    progs = [Gen(p).program() for p in range(1, n + 1)]
    if "--check" in sys.argv:
        want = rust_programs(n)
        bad = [p for p in range(1, n + 1) if progs[p - 1] != want[p - 1]]
        if bad:
            sys.exit(f"{len(bad)} programs differ from the Rust generator, first p={bad[0]}")
        print(f"{n} programs identical to the Rust generator")
    out = os.path.join(HERE, "gen")
    os.makedirs(out, exist_ok=True)
    for p, s in enumerate(progs, 1):
        open(os.path.join(out, f"g{p:03d}.py"), "w").write(s)
    print(f"wrote {n} programs to {out}")


if __name__ == "__main__":
    main()
