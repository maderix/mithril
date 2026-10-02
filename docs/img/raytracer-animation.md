# Animated Cornell renders

The mirror and glass spheres move in all three world coordinates while the
camera stays fixed. Depth changes their apparent size, and the ray tracer
recomputes shadows, reflections and refraction at every pose. Each frame is a
complete render using the original sampling and depth settings.
Whitted renders 512×512 pixels with four samples per pixel; path tracing renders
256×256 pixels with 64 samples per pixel and six surface hits per path.

The measured quantity is **24-frame batch throughput**. FPS is 24 divided by the
median process wall time of three runs after a warmup. CPU uses 16 threads on a
Ryzen 7 7800X3D; GPU uses an RTX 4090. Timings include startup, output formatting
and capture, plus GPU context setup, transfers and readback. The GPU context is
created once for the batch. Compilation, Python pixel validation and GIF encoding
are excluded. The GIFs play at 12.5 FPS independently of rendering throughput.

Every full-resolution run's pixels are checked against a one-thread CPU render.
The small four-frame versions also run against the Core reference interpreter.
The GIF uses a shared 256-colour palette; parity checks use the original RGB
pixels before palette conversion.

## Reproduce

Use a GPU-enabled Mithril CLI and the CUDA setup from the repository README.
The Python helper needs Pillow. It generates source from the existing demo files
and builds it through the normal CLI; no generated Rust or CUDA is hand-edited.

```sh
python3 -m unittest discover -s demos -p test_render_animation.py

# Small independent oracle check on both backends.
python3 demos/render_animation.py --frames 4 --size 8 --spp 2 \
  --samples 1 --oracle --out target/tracer-animation/verify

# Full-resolution animation and timing; build once per backend.
python3 demos/render_animation.py --out target/tracer-animation/full
```

`MITHRIL_NVCC_IMAGE` selects the configured Docker compiler image. The measurement
JSON contains raw samples, FPS, source/compiler/artifact hashes and the pixel
digest. The generated source, frozen CLI and artifacts stay in the output folder.
Published samples are in [raytracer-animation.json](raytracer-animation.json).

The README benchmark rows measure the unchanged static demos. Their current
samples are in [raytracer-static.json](raytracer-static.json); the animation FPS
measures the moving geometry and its 24-frame output.
