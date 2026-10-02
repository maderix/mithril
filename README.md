# Mithril

**Write plain scalar code. Mithril runs it in parallel on every CPU core and on
the GPU, and the result is the same no matter what order the work runs in.**

No threads, no locks, no annotations. Independent work in your program runs at
the same time on its own, and the answer is identical at 1 thread, 16 threads
or on the GPU.

| Whitted ray tracing | Path tracing |
|---|---|
| <img src="docs/img/cornell_whitted_animated.gif" width="320" alt="Moving mirror and glass spheres in the Whitted Cornell scene"> | <img src="docs/img/cornell_path_animated.gif" width="320" alt="Moving mirror and glass spheres in the path-traced Cornell scene"> |
| **CPU 16 threads: 12.6 FPS · GPU: 1.4 FPS** | **CPU 16 threads: 3.2 FPS · GPU: 2.3 FPS** |
| 512×512 · 4 samples per pixel | 256×256 · 64 samples per pixel |

Ryzen 7 7800X3D and RTX 4090; 24-frame batch throughput, median of three runs
with compilation excluded and image return included. Both backends produce
identical pixels. GIF playback is 12.5 FPS. [Measurements and reproduction](docs/img/raytracer-animation.md).

The camera stays fixed while the spheres move in depth and height. Each frame
recomputes their shadows, reflections and refraction. This is
`demos/cornell_path.py`, an ordinary recursive path tracer:

```python
def path(o, d, depth, h, seen):
    x = scene(o, d)
    if x[0] >= far() or depth == 0:
        return (0.0, 0.0, 0.0)
    ...
    direct = next_event(p, n, h)
    more = path(add(p, scale(n, eps())), bounce(n, hash(h + 4)), depth - 1, hash(h + 5), 0)
    return mul(albedo(m), add((direct, direct, direct), more))

def main():
    n = size()
    img = array_new(n * n, 0)
    for y in range(n):
        for x in range(n):
            img = array_set(img, y * n + x, pixel(x, y))
    return (n, n, img)
```

```
mithril run --image cornell.ppm demos/cornell_path.py --threads 1    # 2.149 s
mithril run --image cornell.ppm demos/cornell_path.py                # 0.238 s, 16 threads
mithril run --image cornell.ppm demos/cornell_path.py --gpu          # 0.335 s, RTX 4090
```

All three write the same image, bit for bit (times are run time, excluding
compilation).

[The Mithril Field Guide](docs/guide.html) explains which programs benefit,
the tradeoffs compared with C, Rust, CUDA and functional languages, and how it
works, with diagrams, measurements and links to the proofs and tests.

## How it works

A Mithril program means a set of interaction-net rewrite rules. A rule only
ever touches two nodes, and two rules never touch the same node, so the rules
give the same result in any order: the runtime is free to run them in
parallel, and the compiler is free to run them early.

```
source (Python subset)  ->  Core  ->  net + rules  ->  residual  ->  LIR  ->  CPU: Rust + mithril-rt
mithril-front            oracle    compile-time     what input           GPU: CUDA + engine.cu
                                   reduction        still needs
```

- **The optimizer is the rules.** At compile time every rule that does not
  wait on input fires: constant folding, inlining, branch selection and dead
  code removal are rule firings, not separate passes (`mithril-net`).
- **What remains becomes native code.** Each function is emitted as plain
  native code where it can be, plus forms that can pause and split work
  across workers (`mithril-codegen`).
- **Parallel work comes from the program's shape.** Independent calls,
  proven folds and loops whose iterations are independent run as trees of
  tasks; the result is the sequential one.
- **One runtime model on both devices.** Tasks are pending rule firings,
  records wait for their inputs, dives run native code on a budget
  (`mithril-rt` on the CPU, `engine.cu` on the GPU).

[design.md](docs/design.md) records the full design and its open obligations.

## Build

Requirements: Rust 1.86 or newer and Python 3 (for the test scripts).

```
cargo build --release -p mithril-cli                  # CPU only
cargo build --release -p mithril-cli --features gpu   # CPU and GPU
```

The binary is `target/release/mithril`. `mithril run` compiles the generated
program with `rustc`, so a Rust toolchain must be on the `PATH` when you run
programs too.

GPU support needs an NVIDIA GPU with a CUDA driver, Docker and the NVIDIA
Container Toolkit. The generated CUDA is compiled with `nvcc` inside a Docker
image (CUDA 13.0), so the host needs no CUDA toolkit. Build the image once:

```
docker build -f docker/nvcc.Dockerfile -t mithril-nvcc:cu13.0 docker/
```

`MITHRIL_NVCC_IMAGE` selects another image. Compiled kernels are cached under
`target/mithril-cache/gpu`.

## The language

- Functions (`def`), `if`/`elif`/`else`, `while`, `for i in range(a, b)`,
  `return`, assignment and tuple destructuring (`a, b = t`).
- Integers are 56-bit signed with wrap-around; floats are f64 by default.
  `sqrt(x)` and `f32(n)` make a value binary32, and f32 then spreads through
  the arithmetic; `int(x)` converts back.
- Algebraic data types with `@data`, taken apart with `match`/`case`
  (matches must be exhaustive), tuples and `t[i]` on them.
- Lambdas and closures (`lambda x: x + k`), passed and returned as values.
- Arrays as values: `array_new(n, v)`, `array_get(a, i)`, `array_set(a, i, v)`
  (returns the new array; updated in place when nothing else holds it),
  `array_len(a)`.
- No mutation of shared state, no locks, no annotations for parallelism.

## Examples

- `demos/cornell_whitted.py`, `demos/cornell_path.py`: a Whitted ray tracer
  and a path tracer of the Cornell box. Write the image with
  `mithril run --image cornell.ppm demos/cornell_path.py`.
- `bench/ports/kdtree.py`: nearest-neighbour queries over a shared spatial tree,
  with an independent C implementation and a recorded checksum.
- `bench/general/*.py`: interpreters, persistent maps, pipelines and
  closures, the shapes a general-purpose language has to handle.
- `bench/lockless/`: five programs that need locks or atomics in C and Rust,
  written in Mithril without any, each with C and Rust versions.

## Tests

```
tests/ci/run.sh           # all CPU gates
tests/ci/run.sh --gpu     # plus the GPU gates
```

It runs, in order:

- `cargo test --release --workspace`: unit tests, and every program under
  `crates/*/tests/fixtures` compiled and checked against the reference
  interpreter at 1, 4 and 16 threads and under starved budgets.
- `tests/ci/fast.py`: the spatial-tree benchmark at a small size (exact checksum
  at 1 and 16 threads) plus regression checks on generated-code size and
  instruction counts.
- `tests/parity/check.py`: fixtures, original benchmarks, demos and 500
  generated programs run through the reference interpreter, 1/4/16
  threads, four starved budgets and the GPU, each compared with a pinned
  reference build (`tests/parity/build_ref.py`); `tests/parity/KNOWN.md`
  lists the reference's own known bugs.
- With `--gpu`: `tests/ci/gpu.py` (the ports on the device) and the device
  test suite.

Useful switches: `MITHRIL_STATS=1` (scheduler statistics of a CPU run),
`MITHRIL_TIMING=1` (compile and run times), `MITHRIL_GPU_STATS=1` and
`MITHRIL_GPU_TRACE=1` (per-round device statistics).

## Benchmarks

Recorded wall-clock seconds, compilation excluded. Ryzen 7 7800X3D
(8 cores, 16 threads), RTX 4090. Static render timings are medians of three
runs on 2 October 2026; GPU wall includes context creation and readback.
The spatial-tree CPU measurements are from 29 September and its GPU
measurement from 2 October. All completed lanes agree on their output.

| program | C (1 thread) | Mithril 1 thread | Mithril 16 threads | Mithril GPU (wall) |
|---|---:|---:|---:|---:|
| cornell_path | - | 2.149 s | 0.238 s | 0.335 s |
| cornell_whitted | - | 0.264 s | 0.061 s | 0.871 s |
| kdtree | 0.333 s | 0.406 s | 0.090 s | 1.280 s |

[Spatial-tree CPU methods](bench/results.md) ·
[Render measurements](docs/img/raytracer-static.json).

## Limitations

Generated CPU code is mostly scalar: it targets baseline x86-64 (no AVX or
FMA), and independent calls such as pixels are not vectorized together.

## Repository layout

| path | contents |
|---|---|
| `crates/mithril-front` | lexer, parser, type elaboration, desugaring to Core, the reference interpreter |
| `crates/mithril-core` | ports, agents and the interaction rule table |
| `crates/mithril-net` | compile-time net reduction and specialization |
| `crates/mithril-reassoc` | detection and proof of associative folds |
| `crates/mithril-codegen` | lowering to the IR and the Rust printer |
| `crates/mithril-rt` | the CPU runtime (scheduler, workers, allocator) |
| `crates/mithril-gpu` | the CUDA printer, the device engine (`cuda/engine.cu`) and its host runner |
| `crates/mithril-cli` | the `mithril` command |
| `proofs/confluence` | Lean 4 proof that the core rule table is confluent |
| `bench`, `demos`, `tests` | programs, benchmarks and gates |

## License

Apache License 2.0, see `LICENSE`.
