# fresh arrays a fill writes in full skip their initial value; near misses
# keep it (an element the fill does not write must read as the initial
# value). n is large enough that the device runs the fills as range launches.
def full(n, k):
    a = array_new(n, 7)
    for i in range(n):
        a = array_set(a, i, (i * k + 3) & 4095)
    return a


def short(n, k):
    # the bound is not the length: the last element keeps 7
    a = array_new(n, 7)
    for i in range(n - 1):
        a = array_set(a, i, (i * k) & 4095)
    return a


def offset(n, k):
    # the counter starts at 1: element 0 keeps 7
    a = array_new(n, 7)
    for i in range(1, n):
        a = array_set(a, i, (i * k) & 4095)
    return a


def shifted(n, k):
    # writes at i + 1, not the counter: element 0 keeps 7
    a = array_new(n, 7)
    for i in range(n - 1):
        a = array_set(a, i + 1, (i * k) & 4095)
    return a


def peek(n, k):
    # the array is read before the fill: its initial value is observed
    a = array_new(n, 7)
    first = array_get(a, 0)
    for i in range(n):
        a = array_set(a, i, (i * k + first) & 4095)
    return a


def stencil(g, n):
    # a fill reading another array, as each step of a stencil
    ng = array_new(n, 0)
    for i in range(n):
        ng = array_set(ng, i, (array_get(g, (i + 1) % n) + array_get(g, i)) & 4095)
    return ng


def total(a, n):
    s = 0
    for i in range(n):
        s = (s * 31 + array_get(a, i)) & 4294967295
    return s


def main():
    n = array_len(array_new(300000, 0))
    g = full(n, 5)
    for t in range(3):
        g = stencil(g, n)
    s = short(n, 3)
    o = offset(n, 11)
    h = shifted(n, 13)
    p = peek(n, 17)
    return (total(full(n, 9), n), total(g, n), array_get(s, n - 1), total(s, n), array_get(o, 0), total(o, n), array_get(h, 0), total(h, n), total(p, n))
