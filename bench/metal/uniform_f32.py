# Heterogeneous split benchmark (Metal lane): a uniform binary32 index fill.
# Every element runs the same 256 steps, no recursion and no
# data-dependent branches, so the GPU runs it at full width and the CPU and
# GPU share the range (mithril run uniform_f32.py --threads 10 --metal).


def steps(x):
    y = x * 0.5 + 0.25
    for k in range(256):
        x = x * 0.999 + sqrt(x * x + 1.0) * 0.001
        y = y * 0.998 + x * 0.002
    return x + y


def main():
    n = 4194304
    out = array_new(n, 0)
    for i in range(n):
        out = array_set(out, i, int(steps(f32(i % 4096) * 0.001) * 1000.0))
    return out
