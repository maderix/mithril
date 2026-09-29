# a pair-fork tree whose function returns a tuple (gameoflife's census
# shape): the projections of the first result must not stop the frame
# split, or the tree never forks (both the CPU and the device ran it on
# one worker before)


def prng(x):
    b = x ^ ((x << 13) & 4294967295)
    d = b ^ (b >> 17)
    return d ^ ((d << 5) & 4294967295)


def leaf(i, g):
    pa = 0
    mx = 0
    for j in range(64):
        v = prng((i + j) * 2654435761 & 4294967295)
        k = 0
        while k < g:
            v = prng(v)
            k = k + 1
        pa = (pa + (v & 255)) & 4294967295
        mx = ((mx * 2654435761) & 4294967295) ^ v
    return (pa, mx)


def zip2(a, b):
    return ((a[0] + b[0]) & 4294967295, (a[1] * 2654435761 + b[1]) & 4294967295)


def tree(d, i, g):
    if d == 0:
        return leaf(i, g)
    a = tree(d - 1, i, g)
    b = tree(d - 1, i + (64 << (d - 1)), g)
    return zip2(a, b)


def main():
    t = tree(10, 0, 8)
    return (t[0] * 2654435761 + t[1]) & 4294967295
