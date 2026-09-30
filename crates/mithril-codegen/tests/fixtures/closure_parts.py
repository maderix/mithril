# Components and elements read out of a closure result (untyped): a
# function that also takes ints must not read them as ints.
def ap(f, s, n):
    if n == 0:
        return f(s)
    return ap(f, s, n - 1)


def dbl(x, n):
    if n == 0:
        return x + x
    return dbl(x, n - 1)


def main():
    n = array_len(array_new(3, 0))
    s = array_get(array_new(n, 2.5), 0)
    t = ap(lambda y: (y, y), s, n)
    a = ap(lambda y: array_new(2, y), s, n)
    return (dbl(t[0], n), dbl(array_get(a, 1), n), dbl(n, n))
