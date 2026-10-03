# heat2d: heat diffusion on a square grid of fixed-point temperatures. Each
# time step computes every interior cell from itself and its four
# neighbours in the previous grid, so the cells of one step are independent
# and the steps run one after another. The border keeps its starting
# value. Starting temperatures are xorshift hashes of the cell index.
# Checksum: the sum of the final grid, u32. Sizes: small w 32, steps 10;
# big w 1024, steps 500.


def prng(x):
    b = x ^ ((x << 13) & 4294967295)
    d = b ^ (b >> 17)
    return d ^ ((d << 5) & 4294967295)


def start(w):
    g = array_new(w * w, 0)
    for i in range(w * w):
        g = array_set(g, i, (prng((i + 1) * 2654435761 & 4294967295) & 1023) * 256)
    return g


def cell(g, i, w):
    x = i % w
    y = i // w
    if x == 0 or y == 0 or x == w - 1 or y == w - 1:
        return array_get(g, i)
    c = array_get(g, i)
    up = array_get(g, i - w)
    down = array_get(g, i + w)
    left = array_get(g, i - 1)
    right = array_get(g, i + 1)
    return (4 * c + up + down + left + right) // 8


def step(g, w):
    ng = array_new(w * w, 0)
    for i in range(w * w):
        ng = array_set(ng, i, cell(g, i, w))
    return ng


def total(g, n):
    s = 0
    for i in range(n):
        s = (s + array_get(g, i)) & 4294967295
    return s


def run(w, steps):
    g = start(w)
    for t in range(steps):
        g = step(g, w)
    return total(g, w * w)


def main():
    return run(1024, 500)
