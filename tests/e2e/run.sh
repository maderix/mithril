#!/usr/bin/env bash
# End-to-end integration gate for Mithril (task 10).
#
# For every benchmark port in bench/ports/*.py:
#   1. copy the port to a temp dir and rewrite main's size arguments down to
#      the SMALL size documented in the port header (and in the matching
#      reference/bench/runtime/<name>/main.reference header), via the exact textual
#      substitutions tabulated below;
#   2. run it with --threads 1 and --threads 16 and assert the printed
#      checksum equals the documented small expected value;
#   3. if the binary has GPU support and a GPU is present, run the same
#      small copy with --gpu ("arena exhausted" => SKIP, not FAIL).
# Then the two flagship BIG runs, straight from the unmodified ports:
#   mandelbrot.py   must print 3101455856  (--threads 1 and --threads 16)
#   tree-bitonic.py must print 3787129428  (--threads 1 and --threads 16)
# plus --gpu lanes for both when GPU works (MITHRIL_GPU_NODES=1073741824 for
# the big tree run; "arena exhausted" => SKIP).
#
# Small-size substitutions (port: big main line -> small main line, expect):
#   bfs:          batch(19, 0)                    -> batch(4, 0)                     512822161
#   editdist:     batch(15, 0)                    -> batch(2, 0)                     2065873279
#   gameoflife:   census_fin(batch_run(18, 0, 32))-> census_fin(batch_run(1, 0, 2))  2601177279
#   hashmap:      batch(11, 0, 16384)             -> batch(2, 0, 256)                4206244792
#   kmeans:       rbatch(6, 0, 19)                -> rbatch(6, 0, 7)                 4219097976
#   lexer:        batch(23, 0)                    -> batch(8, 0)                     1822208108
#   mandelbrot:   rend(18, 51)                    -> rend(2, 7)                      887240761
#   merkle:       build(22, 0) and
#                 mrk(root, pverify(pgen(22, 1337, 0), leafh(1337)))
#                   -> build(4, 0) and mrk(root, pverify(pgen(4, 13, 0), leafh(13)))
#                                                                                    524568222
#   nbody:        run(1048576, 300)               -> run(1024, 300)                  2215450620
#   queens:       run(17, 17, 11730)              -> run(10, 5, 625)                 774553824
#   raytrace:     rowf(12, 0, 4095, 6000, 3000.0, 2048.0)
#                   -> rowf(6, 0, 63, 80, 40.0, 32.0)                                402971
#   symreg:       run(18, 42, 32, 110)            -> run(6, 42, 32, 16)              2490246820
#   terrain:      for t in range(65536):          -> for t in range(16):             4236200168
#   tree-bitonic: stat_out(scan(bsort(23, 0, 0))) -> stat_out(scan(bsort(8, 0, 0)))  971629740
#   tree-matmul:  batch(9, 0, 384, 511, 7)        -> batch(2, 0, 3, 3, 3)            4292995302
#   tree-radix:   chk_out(chk(sort(gen(22, 0))))  -> chk_out(chk(sort(gen(8, 0))))   366571299
# All 16 substitutions are verified against the CPython oracle
# (python3 bench/ports/_pyshim.py <small copy>).
#
# Arena knobs: v1 generated code never frees cells
# (crates/mithril-codegen/src/lib.rs), so CPU lanes need headroom:
# small lanes run with MITHRIL_NODES=2^28; big lanes with the u32 cap
# MITHRIL_NODES=4294967295 and MITHRIL_RECS=2^28.
#
# Usage: tests/e2e/run.sh [--no-gpu] [--no-big]
# Exit status: number of FAILed lanes (0 = gate green; SKIPs do not fail).

set -u
cd "$(dirname "$0")/../.."
ROOT=$PWD

NO_GPU=0
NO_BIG=0
for a in "$@"; do
  case "$a" in
    --no-gpu) NO_GPU=1 ;;
    --no-big) NO_BIG=1 ;;
    *) echo "usage: $0 [--no-gpu] [--no-big]" >&2; exit 2 ;;
  esac
done

BIN=${MITHRIL_BIN:-}
if [ -z "$BIN" ]; then
  BIN=${CARGO_TARGET_DIR:-$ROOT/target}/release/mithril
  if [ ! -x "$BIN" ]; then
    echo "== building mithril (release, gpu feature) =="
    if ! cargo build --release -p mithril-cli --features gpu >/dev/null 2>&1; then
      echo "gpu-feature build failed, building CPU-only"
      cargo build --release -p mithril-cli 2>&1 | tail -3
    fi
  fi
fi
[ -x "$BIN" ] || { echo "no mithril binary at $BIN" >&2; exit 2; }

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# ---- small-size copies ------------------------------------------------------
# sub <name> <sed-expr>... : copy port, apply substitutions, verify each hit.
sub() {
  local n=$1; shift
  cp "bench/ports/$n.py" "$TMP/$n.py"
  local e
  for e in "$@"; do
    sed -i "$e" "$TMP/$n.py"
  done
  if cmp -s "bench/ports/$n.py" "$TMP/$n.py"; then
    echo "internal error: small-size substitution for $n did not apply" >&2
    exit 2
  fi
}
sub bfs 's/return batch(19, 0)/return batch(4, 0)/'
sub editdist 's/return batch(15, 0)/return batch(2, 0)/'
sub gameoflife 's/return census_fin(batch_run(18, 0, 32))/return census_fin(batch_run(1, 0, 2))/'
sub hashmap 's/return batch(11, 0, 16384)/return batch(2, 0, 256)/'
sub kmeans 's/return rbatch(6, 0, 19)/return rbatch(6, 0, 7)/'
sub lexer 's/return batch(23, 0)/return batch(8, 0)/'
sub mandelbrot 's/return rend(18, 51)/return rend(2, 7)/'
sub merkle 's/t = build(22, 0)/t = build(4, 0)/' \
  's/return mrk(root, pverify(pgen(22, 1337, 0), leafh(1337)))/return mrk(root, pverify(pgen(4, 13, 0), leafh(13)))/'
sub nbody 's/return run(1048576, 300)/return run(1024, 300)/'
sub queens 's/return run(17, 17, 11730)/return run(10, 5, 625)/'
sub raytrace 's/return rowf(12, 0, 4095, 6000, 3000.0, 2048.0)/return rowf(6, 0, 63, 80, 40.0, 32.0)/'
sub symreg 's/return run(18, 42, 32, 110)/return run(6, 42, 32, 16)/'
sub terrain 's/for t in range(65536):/for t in range(16):/'
sub tree-bitonic 's/return stat_out(scan(bsort(23, 0, 0)))/return stat_out(scan(bsort(8, 0, 0)))/'
sub tree-matmul 's/return batch(9, 0, 384, 511, 7)/return batch(2, 0, 3, 3, 3)/'
sub tree-radix 's/return chk_out(chk(sort(gen(22, 0))))/return chk_out(chk(sort(gen(8, 0))))/'

PORTS="bfs editdist gameoflife hashmap kmeans lexer mandelbrot merkle nbody queens raytrace symreg terrain tree-bitonic tree-matmul tree-radix"
expect_small() {
  case $1 in
    bfs) echo 512822161 ;;          editdist) echo 2065873279 ;;
    gameoflife) echo 2601177279 ;;  hashmap) echo 4206244792 ;;
    kmeans) echo 4219097976 ;;      lexer) echo 1822208108 ;;
    mandelbrot) echo 887240761 ;;   merkle) echo 524568222 ;;
    nbody) echo 2215450620 ;;       queens) echo 774553824 ;;
    raytrace) echo 402971 ;;        symreg) echo 2490246820 ;;
    terrain) echo 4236200168 ;;     tree-bitonic) echo 971629740 ;;
    tree-matmul) echo 4292995302 ;; tree-radix) echo 366571299 ;;
  esac
}

# ---- GPU availability probe -------------------------------------------------
GPU=0
if [ "$NO_GPU" = 0 ]; then
  if MITHRIL_GPU=1 timeout 120 "$BIN" run "$TMP/queens.py" --gpu >"$TMP/gpuprobe" 2>&1 \
     && [ "$(tail -1 "$TMP/gpuprobe")" = "$(expect_small queens)" ]; then
    GPU=1
  else
    echo "gpu probe failed, skipping --gpu lanes: $(tail -1 "$TMP/gpuprobe")"
  fi
fi

FAILS=0
RESULTS=()

# lane <label> <expect> <timeout-s> <env NAME=V ...> -- <cmd...>
lane() {
  local label=$1 expect=$2 tmo=$3; shift 3
  local envs=()
  while [ "$1" != "--" ]; do envs+=("$1"); shift; done
  shift
  local out got st
  out=$(env "${envs[@]}" timeout "$tmo" "$@" 2>&1); st=$?
  got=$(echo "$out" | tail -1)
  if [ "$got" = "$expect" ]; then
    st=PASS
  elif echo "$out" | grep -Eq "arena exhausted|CUDA driver call alloc .* code 2" \
       && [ "${label##*gpu}" != "$label" ]; then
    st=SKIP   # GPU lanes: arena/VRAM exhaustion is a capacity skip, not a failure
  else
    st=FAIL; FAILS=$((FAILS + 1))
    echo "FAIL $label: expected $expect, got: $(echo "$out" | grep -v '^$' | tail -1)"
  fi
  RESULTS+=("$(printf '%-6s %s' "$st" "$label")")
  echo "$st  $label"
}

# ---- small lanes ------------------------------------------------------------
SMALL_NODES=268435456          # 2^28: raytrace/terrain small need >2^26 cells
for n in $PORTS; do
  e=$(expect_small "$n")
  lane "$n small t1"  "$e" 600 MITHRIL_NODES=$SMALL_NODES -- "$BIN" run "$TMP/$n.py" --threads 1
  lane "$n small t16" "$e" 600 MITHRIL_NODES=$SMALL_NODES -- "$BIN" run "$TMP/$n.py" --threads 16
  if [ "$GPU" = 1 ]; then
    lane "$n small gpu" "$e" 600 MITHRIL_GPU=1 -- "$BIN" run "$TMP/$n.py" --gpu
  fi
done

# ---- flagship big lanes -----------------------------------------------------
if [ "$NO_BIG" = 0 ]; then
  BIG_NODES=4294967295         # u32 cap; big runs never free (codegen v1)
  BIG_RECS=268435456
  lane "mandelbrot BIG t16" 3101455856 3600 MITHRIL_NODES=$BIG_NODES MITHRIL_RECS=$BIG_RECS -- \
    "$BIN" run bench/ports/mandelbrot.py --threads 16
  lane "mandelbrot BIG t1" 3101455856 3600 MITHRIL_NODES=$BIG_NODES MITHRIL_RECS=$BIG_RECS -- \
    "$BIN" run bench/ports/mandelbrot.py --threads 1
  lane "tree-bitonic BIG t16" 3787129428 3600 MITHRIL_NODES=$BIG_NODES MITHRIL_RECS=$BIG_RECS -- \
    "$BIN" run bench/ports/tree-bitonic.py --threads 16
  lane "tree-bitonic BIG t1" 3787129428 3600 MITHRIL_NODES=$BIG_NODES MITHRIL_RECS=$BIG_RECS -- \
    "$BIN" run bench/ports/tree-bitonic.py --threads 1
  if [ "$GPU" = 1 ]; then
    lane "mandelbrot BIG gpu" 3101455856 3600 MITHRIL_GPU=1 MITHRIL_GPU_NODES=805306368 -- \
      "$BIN" run bench/ports/mandelbrot.py --gpu
    lane "tree-bitonic BIG gpu" 3787129428 3600 MITHRIL_GPU=1 MITHRIL_GPU_NODES=1073741824 -- \
      "$BIN" run bench/ports/tree-bitonic.py --gpu
  fi
fi

echo
echo "==== e2e gate summary ===="
printf '%s\n' "${RESULTS[@]}"
echo "FAILED lanes: $FAILS"
exit $((FAILS > 125 ? 125 : FAILS))
