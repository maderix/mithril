@data
class L:
    Nil: ()
    Cons: (h, t)


def ap(f, v):
    return f(v)


def pair(a, b):
    return (a, b)


def build(n, s):
    if n == 0:
        return Nil()
    return Cons(s + n, build(n - 1, s))


def total(l):
    match l:
        case Nil():
            return 0
        case Cons(h, t):
            return h + total(t)


def h0(a, b):
    x = pair(pair(12, 6)[0], ((b ^ a) if total(build(0, 11)) < total(build(2, a)) else 3))[0]
    return ((4 & pair(x, 7)[0]) if (ap(lambda x: x * 9 + 4, 12) & ap(lambda x: x * x + 4, 6)) < total(build(2, (a & 4))) else 6)


def h1(a, b):
    x = total(build(3, (h0(a, 2) if ap(lambda x: x * 13 + 0, b) < total(build(2, 18)) else h0(13, 13))))
    return a


def h2(a, b):
    x = h1(15, total(build(1, pair(a, a)[1])))
    return a


def main():
    y = (h0(14, ap(lambda x: x * 10 + 1, 5)) + pair(10, ap(lambda x: x * 11 + 6, 12))[0])
    f = lambda x: x * y + y
    l = build(1, y)
    return (total(build(0, y)), (y ^ pair(8, ap(lambda x: x * y + 6, y))[0]), f(total(build(0, y))) + f(y), total(l) + total(l))
