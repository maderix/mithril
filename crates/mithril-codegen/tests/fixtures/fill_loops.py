# index fills: loops writing an array once per index, run as chunks that
# write one buffer in place; and near misses that must stay sequential
def tab(n, k):
    a = array_new(n, 0)
    for i in range(n):
        a = array_set(a, i, (i * k + 7) & 1023)
    return a


def stencil(g, n):
    ng = array_new(n, 0)
    for i in range(n):
        left = array_get(g, (i + n - 1) % n)
        right = array_get(g, (i + 1) % n)
        ng = array_set(ng, i, (left + 2 * array_get(g, i) + right) & 65535)
    return ng


def refill(a, lo, hi, k):
    # `a` is still read by the caller afterwards: the fill must copy it
    for i in range(lo, hi):
        a = array_set(a, i, (i * k) & 255)
    return a


def carried(n):
    # x carries a value between iterations: not a fill
    a = array_new(n, 0)
    x = 1
    for i in range(n):
        x = (x * 3 + i) & 65535
        a = array_set(a, i, x)
    return a


def bump(a, n):
    # reads the array it writes: not a fill
    for i in range(n):
        a = array_set(a, i, array_get(a, i) + 1)
    return a


def total(a, n):
    s = 0
    for i in range(n):
        s = (s * 31 + array_get(a, i)) & 4294967295
    return s


def main():
    n = array_len(array_new(3000, 0))
    a = tab(n, 13)
    g = a
    for t in range(4):
        g = stencil(g, n)
    b = refill(a, 100, 2000, 7)
    c = carried(n)
    d = bump(tab(n, 5), n)
    return (total(a, n), total(g, n), total(b, n), total(c, n), total(d, n), array_get(a, 150))
