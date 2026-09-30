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
    x = 5
    return 8


def h1(a, b):
    x = 14
    return (pair(h0(8, x), (0 if 19 < b else 0))[0] * ((a + 18) if (x & 14) < pair(a, a)[0] else h0(x, x)))


def h2(a, b):
    x = pair(pair((b - b), h0(b, a))[1], a)[1]
    return total(build(3, (total(build(3, 17)) + ap(lambda x: x * a + 0, 12))))


def main():
    y = 2
    f = lambda x: x * y + y
    l = build(2, y)
    return (3, (y + y), f(3) + f(y), total(l) + total(l))
