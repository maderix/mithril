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
    x = ap(lambda x: x * ap(lambda x: x * (b * b) + 8, ap(lambda x: x * b + 3, a)) + 0, pair(total(build(6, 5)), 3)[0])
    return b


def h1(a, b):
    x = b
    return (x & 2)


def h2(a, b):
    x = ap(lambda x: x * ((12 if 3 < b else 13) & a) + 7, pair((b if a < a else 3), ap(lambda x: x * b + 3, a))[0])
    return pair((pair(x, 2)[0] ^ x), ap(lambda x: x * total(build(6, b)) + 6, total(build(4, b))))[1]


def main():
    y = total(build(5, ((10 if 3 < 13 else 19) - pair(2, 0)[0])))
    f = lambda x: x * y + y
    l = build(1, y)
    return (y, h0(h0(h0(15, y), total(build(3, y))), ap(lambda x: x * pair(14, 15)[0] + 1, h1(y, 18))), f(y) + f(y), total(l) + total(l))
