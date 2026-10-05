#!/usr/bin/env bash
# Mithril CI: build, unit and integration tests, the port gate and the parity
# check against the pinned reference binary.
#
#   tests/ci/run.sh          CPU gates (a few minutes)
#   tests/ci/run.sh --gpu    also the device gates (needs an NVIDIA GPU and
#                            the nvcc docker image, see README.md)
#
# Exits non-zero on the first failing gate.
set -euo pipefail

cd "$(dirname "$0")/../.."
gpu=0
[[ "${1:-}" == "--gpu" ]] && gpu=1

step() { printf '\n== %s\n' "$*"; }

step "unit and integration tests"
cargo test --release --workspace

step "build the CLI"
cargo build --release -p mithril-cli --bins --example dump_gen

step "port gate (tests/ci/fast.py)"
python3 tests/ci/fast.py

step "parity reference binary"
[[ -x target/parity-ref/mithril ]] || python3 tests/parity/build_ref.py

lanes=cpu
((gpu)) && lanes=oracle,run_t1,run_t4,run_t16,fuel1,fuel2,fuel7,fuel64,device
step "parity check (tests/parity/check.py, lanes: $lanes)"
python3 tests/parity/check.py --lanes "$lanes"

if ((gpu)); then
    step "device gate (tests/ci/gpu.py)"
    python3 tests/ci/gpu.py
    step "device tests"
    MITHRIL_GPU=1 cargo test --release -p mithril-gpu --test gpu_test -- --ignored --test-threads=1
    MITHRIL_GPU=1 cargo test --release -p mithril-cli
fi

step "all gates passed"
