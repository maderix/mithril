#!/usr/bin/env bash
# CPU and optional device checks of independently authored programs.
set -euo pipefail
cd "$(dirname "$0")/../.."
pwd
python3 tests/ci/fast.py
if [[ " $* " != *" --no-gpu "* ]]; then
    python3 tests/ci/gpu.py
fi
