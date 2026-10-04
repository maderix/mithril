#!/usr/bin/env python3
"""Render the black hole flight (demos/black_hole.py) as PNG frames.

The demo renders a strip of frames per run, starting at first_frame(). This
helper writes one source per strip, builds each through the normal CLI, runs
it and splits the returned strip into frames. With --check it first renders a
small strip on the reference interpreter, one CPU thread, 16 threads and the
GPU, and stops unless all four agree.

    python3 demos/render_black_hole.py --out target/black_hole
    ffmpeg -framerate 30 -i target/black_hole/frame_%03d.png hole.mp4

Needs Pillow; --check also needs a CPU build; the GPU needs the CUDA setup
from the README.
"""
import argparse
import ast
import hashlib
import os
import re
import subprocess
import sys

DEMO = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'black_hole.py')


def with_constants(src, **values):
    # replace the body of each zero-argument constant function, e.g. width()
    for name, value in values.items():
        src, n = re.subn(rf'(def {name}\(\):\n    return ).*', rf'\g<1>{value}', src)
        if n != 1:
            sys.exit(f'{name}() not found in {DEMO}')
    return src


def run(cmd, **kw):
    return subprocess.run(cmd, check=True, capture_output=True, text=True, **kw)


def check(cli, out):
    src = with_constants(open(DEMO).read(), width=32, height=20, frames=2, first_frame=300)
    path = os.path.join(out, 'check.py')
    open(path, 'w').write(src)
    digest = lambda text: hashlib.md5(text.encode()).hexdigest()
    got = {'oracle': digest(run([cli, 'oracle', path]).stdout)}
    run([cli, 'build', path, '-o', path + '.cpu'])
    for t in (1, 16):
        got[f'cpu t{t}'] = digest(run([path + '.cpu', '--threads', str(t)]).stdout)
    run([cli, 'build', '--gpu', path, '-o', path + '.gpu'])
    got['gpu'] = digest(run([cli, 'exec', path + '.gpu']).stdout)
    for k, v in got.items():
        print(f'{k:8} {v}')
    if len(set(got.values())) != 1:
        sys.exit('lanes disagree')


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--out', default='target/black_hole')
    ap.add_argument('--cli', default='target/release/mithril')
    ap.add_argument('--strip', type=int, default=40, help='frames per run')
    ap.add_argument('--check', action='store_true', help='check parity on a small strip first')
    ap.add_argument('--limit', type=int, help='render only the first LIMIT frames')
    args = ap.parse_args()
    from PIL import Image

    os.makedirs(args.out, exist_ok=True)
    if args.check:
        check(args.cli, args.out)
    demo = open(DEMO).read()
    total = int(re.search(r'def flight_frames\(\):\n    return (\d+)', demo).group(1))
    total = min(total, args.limit or total)
    width = int(re.search(r'def width\(\):\n    return (\d+)', demo).group(1))
    height = int(re.search(r'def height\(\):\n    return (\d+)', demo).group(1))
    for first in range(0, total, args.strip):
        n = min(args.strip, total - first)
        path = os.path.join(args.out, f'strip_{first:03d}.py')
        open(path, 'w').write(with_constants(demo, frames=n, first_frame=first))
        run([args.cli, 'build', '--gpu', path, '-o', path + '.gpu'])
        res = run([args.cli, 'exec', path + '.gpu'], env=dict(os.environ, MITHRIL_GPU_STATS='1'))
        w, h, px = ast.literal_eval(res.stdout.strip())
        assert (w, h) == (width, height * n)
        for i in range(n):
            frame = px[i * w * height:(i + 1) * w * height]
            im = Image.new('RGB', (w, height))
            im.putdata([((p >> 16) & 255, (p >> 8) & 255, p & 255) for p in frame])
            im.save(os.path.join(args.out, f'frame_{first + i:03d}.png'))
        ms = re.search(r'run (\d+) ms', res.stderr)
        print(f'frames {first}-{first + n - 1}: device run {ms.group(1) if ms else "?"} ms')


if __name__ == '__main__':
    main()
