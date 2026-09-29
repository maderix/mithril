# Nested tuples in native code: a record (a, (b, c), d) built, passed,
# returned and projected by native functions, and crossing the boundary
# to the forking (dive-form) tree in both directions: its leaves come
# back packed as nested tuple cells, and main prints one.
def make(i):
    return (i, (i * 2, i * 3), i + 1)


def use(h):
    n = h[1]
    return h[0] + n[0] * 10 + n[1] * 100 + h[2] * 1000


def pick(h, k):
    if use(k) < use(h):
        return k
    return h


def shift(h, s):
    n = h[1]
    return (h[0] + s, (n[1], n[0] + s), h[2])


def tree(lo, n):
    if n == 1:
        return shift(make(lo), lo & 3)
    a = tree(lo, n // 2)
    b = tree(lo + n // 2, n - n // 2)
    return pick(a, b)


def main():
    h = tree(5, array_len(array_new(64, 0)))
    return (use(h), h)
