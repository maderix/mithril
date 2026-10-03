# proven folds that read a shared array in every iteration: the array is
# lent to the fold by its caller, so each split chunk takes its own
# reference; one fold keeps an int accumulator, one an elementwise tuple
def table(n, seed):
    a = array_new(n, 0)
    x = seed
    for i in range(n):
        x = (x * 1103515245 + 12345) & 2147483647
        a = array_set(a, i, x >> 8)
    return a


def mix(i, v):
    h = (i * 2654435761 + v) & 4294967295
    return (h ^ (h >> 15)) & 65535


def digest(a, n):
    h = 0
    for i in range(n):
        h = (h + mix(i, array_get(a, i))) & 4294967295
    return h


def hadd(p, q):
    return ((p[0] + q[0]) & 4294967295, (p[1] + q[1]) & 4294967295)


def one(v):
    if v & 1 == 0:
        return (1, 0)
    return (0, 1)


def parity(a, n):
    c = (0, 0)
    for i in range(n):
        c = hadd(c, one(array_get(a, i)))
    return c


def main():
    n = array_len(array_new(3000, 0))
    a = table(n, 7)
    p = parity(a, n)
    return (digest(a, n), p[0], p[1], array_get(a, n - 1))
