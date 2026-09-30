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
    x = (pair(b, pair(a, 2)[0])[0] if (16 & pair(a, a)[0]) < total(build(4, (a + a))) else pair(total(build(4, a)), 13)[1])
    return (pair(pair(19, b)[1], total(build(2, b)))[0] * 19)


def h1(a, b):
    x = total(build(3, h0(0, (b & 3))))
    return a


def h2(a, b):
    x = pair((ap(lambda x: x * 5 + 8, b) if h1(4, b) < pair(16, a)[1] else b), pair(pair(17, b)[1], b)[0])[1]
    return pair((pair(2, 6)[0] - 14), x)[0]


def main():
    y = h1(13, pair(pair(13, 12)[1], h0(0, 9))[1])
    f = lambda x: x * h2(ap(lambda x: x * 14 + 0, 11), (17 if 1 < 2 else y)) + y
    l = build(0, y)
    return (9, total(build(1, (y & 17))), f(9) + f(y), total(l) + total(l))
