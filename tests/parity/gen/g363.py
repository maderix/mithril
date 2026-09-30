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
    x = 3
    return pair(ap(lambda x: x * 5 + 8, (a if 18 < 3 else a)), (x & total(build(6, 18))))[1]


def h1(a, b):
    x = h0((ap(lambda x: x * 16 + 2, 12) + total(build(4, 9))), h0(pair(b, a)[0], total(build(3, a))))
    return x


def h2(a, b):
    x = 1
    return 11


def main():
    y = (pair(total(build(2, 2)), pair(2, 9)[1])[0] * (h0(3, 12) & (18 & 0)))
    f = lambda x: x * y + y
    l = build(3, y)
    return (total(build(2, pair((y if y < 16 else 10), h1(19, 6))[1])), h2(pair(total(build(1, y)), h1(15, 1))[0], total(build(2, ap(lambda x: x * 6 + 2, y)))), f(total(build(2, pair((y if y < 16 else 10), h1(19, 6))[1]))) + f(y), total(l) + total(l))
