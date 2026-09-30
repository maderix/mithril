#!/usr/bin/env python3
"""Build the pinned parity reference: today's system at commit REF_COMMIT,
plus the one product change the harness needs (`mithril oracle f.py`,
tests/parity/ref_oracle.patch; the same hook is in the tree from the
harness commit on). Result: target/parity-ref/mithril.

The worktree lives at target/parity-ref/src (its own target dir under it),
so building the reference never touches the working tree or its cache.
The reference's rt cache (target/mithril-cache of the worktree) is keyed
by its own sources, so `mithril run/build` of the reference compiles
against the reference runtime.

Usage: tests/parity/build_ref.py [--force]
"""
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
REF_COMMIT = "21e18d8"
REF_DIR = os.path.join(ROOT, "target", "parity-ref")
SRC = os.path.join(REF_DIR, "src")
BIN = os.path.join(REF_DIR, "mithril")


def sh(cmd, **kw):
    print("+", " ".join(cmd), flush=True)
    subprocess.run(cmd, check=True, **kw)


def main():
    force = "--force" in sys.argv[1:]
    if force and os.path.exists(SRC):
        sh(["git", "worktree", "remove", "--force", SRC], cwd=ROOT)
    if not os.path.exists(SRC):
        os.makedirs(REF_DIR, exist_ok=True)
        sh(["git", "worktree", "add", "--detach", SRC, REF_COMMIT], cwd=ROOT)
        sh(["git", "apply", os.path.join(HERE, "ref_oracle.patch")], cwd=SRC)
    head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=SRC, capture_output=True, text=True).stdout.strip()
    if not REF_COMMIT.startswith(head) and not head.startswith(REF_COMMIT):
        sys.exit(f"{SRC} is at {head}, not {REF_COMMIT}; rerun with --force")
    # the binary locates its runtime sources through CARGO_MANIFEST_DIR
    # (the worktree) and caches under its own target dir
    env = dict(os.environ)
    env.pop("CARGO_TARGET_DIR", None)
    sh(["cargo", "build", "--release", "-p", "mithril-cli", "--features", "gpu"], cwd=SRC, env=env)
    shutil.copy2(os.path.join(SRC, "target", "release", "mithril"), BIN)
    print(f"reference: {BIN} ({REF_COMMIT} + oracle hook)")


if __name__ == "__main__":
    main()
