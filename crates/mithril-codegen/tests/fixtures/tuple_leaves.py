# Tuples passed whole whose leaves are not all ints: an array, a
# constructor, an f64, and nesting past the layout bound (9 levels). Their
# functions must not treat the leaves as ints (no native tuple layout).
@data
class Opt:
    No: ()
    Some: (v,)


def keep(p, n):
    if n == 0:
        return p
    return keep(p, n - 1)


def total(p):
    a = p[1]
    s = 0
    for i in range(array_len(a)):
        s = s + array_get(a, i)
    return s + p[0]


def get(o):
    match o:
        case No():
            return 0
        case Some(v):
            return v


def mk(n):
    return (n, (n, (n, (n, (n, (n, (n, (n, (n + 1, n + 2)))))))))


def walk(t):
    a = t[1]
    b = a[1]
    c = b[1]
    d = c[1]
    e = d[1]
    f = e[1]
    g = f[1]
    h = g[1]
    return h[0] * 100 + h[1] + t[0]


def keep2(p, n):
    if n == 0:
        return p
    return keep2(p, n - 1)


def keep3(p, n):
    if n == 0:
        return p
    return keep3(p, n - 1)


def keep4(p, n):
    if n == 0:
        return p
    return keep4(p, n - 1)


def main():
    n = array_len(array_new(3, 0))
    arr = total(keep((7, array_new(n + 2, 3)), n))
    adt = keep2((7, Some(n + 40)), n)
    flo = keep3((7, 2.5), n)
    deep = walk(keep4(mk(n), n))
    return (arr, get(adt[1]) + adt[0], flo, deep)
