# A result typed int by one caller, a float at runtime through another:
# the op on the float parameter must relink the result type (walk order
# must not decide it).
def c0(n, d):
    if f2(n, d) == 0:
        return 1
    return 0


def c1(n):
    return f2(n - 1, n)


def k(y, e):
    return f2(y, e) + f2(y, e)


def f2(x, d):
    if d == 0:
        return x + x
    return f2(x, d - 1)


def main():
    a = array_len(array_new(3, 0))
    b = array_len(array_new(3, 0))
    c = array_len(array_new(3, 0))
    d = array_len(array_new(3, 0))
    e = array_len(array_new(3, 0))
    g = array_get(array_new(a, 2.5), 0)
    r = k(g, c)
    return (c0(d, e), c1(b), r * r)
