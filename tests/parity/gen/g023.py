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
    x = 6
    return x


def h1(a, b):
    x = pair(pair(h0(14, b), (a - a))[0], 10)[1]
    return ap(lambda x: x * total(build(4, (5 if 7 < b else b))) + 7, total(build(4, pair(b, 8)[0])))


def h2(a, b):
    x = total(build(0, ap(lambda x: x * a + 3, pair(18, 0)[1])))
    return pair(total(build(5, (x * 19))), x)[0]


def main():
    y = h2(ap(lambda x: x * (17 if 3 < 18 else 19) + 8, (3 if 8 < 6 else 19)), (pair(7, 19)[1] * h2(9, 8)))
    f = lambda x: x * h2(total(build(3, 11)), y) + y
    l = build(1, y)
    return (y, h2((pair(1, 19)[1] if 16 < (y & 1) else y), pair((15 + 8), (y * 15))[1]), f(y) + f(y), total(l) + total(l))
