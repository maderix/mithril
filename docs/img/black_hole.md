# Black hole flight

[`demos/black_hole.py`](../../demos/black_hole.py) renders a black hole
spinning at 0.99 of the maximum, seen from a camera that descends on circular
orbits from 26 to 6 gravitational radii, two degrees above the accretion disk.
Each pixel's light ray is traced backward through the Kerr spacetime with
fourth-order Runge-Kutta steps; the disk is a blackbody seen at its shifted
temperature, and the camera's motion bends and shifts the starlight. The flight
is 450 frames of 1280×720.

The image is made in passes, each one loop over its pixels: one ray per pixel in
high dynamic range, four more rays for pixels that differ sharply from a
neighbour, a glare pass for the brightest light, then a filmic tone curve.

## Measurements

RTX 4090 and Ryzen 7 7800X3D, 4 October 2026.

| | Time |
|---|---|
| GPU, 40-frame strips, device time per frame | 16–31 ms (32–62 FPS) |
| One frame (frame 225), 1 thread | 11.38 s |
| One frame, 16 threads | 1.24 s |
| One frame, GPU wall, context and readback included | 0.25 s |

Per-strip device times, frames 0–39 to 440–449 (ms): 1169, 1219, 1229, 1243,
1211, 1033, 746, 725, 712, 644, 657, 211. Frames near the start cost more: the
star field makes more pixels take the extra edge rays. The one-frame rows are
process wall times; 16 threads and the GPU are medians of three runs, 1 thread
is one run. The three one-frame images are identical, and a 32×20 two-frame
strip is identical on the reference interpreter, 1 thread, 16 threads and the
GPU.

## Reproduce

```sh
python3 demos/render_black_hole.py --check --out target/black_hole
ffmpeg -framerate 30 -i target/black_hole/frame_%03d.png -pix_fmt yuv420p hole.mp4
```

`--check` runs the small parity strip first. `--limit N` renders only the first
N frames.
