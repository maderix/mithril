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
    x = ap(lambda x: x * 13 + 1, pair((0 if 3 < a else 7), (2 if a < 1 else a))[1])
    return ap(lambda x: x * ((1 + a) if total(build(5, b)) < (b if b < 17 else 17) else x) + 0, (ap(lambda x: x * 12 + 6, 3) - ap(lambda x: x * 9 + 8, 6)))


def h1(a, b):
    x = ap(lambda x: x * pair((a * 8), (2 ^ 13))[1] + 1, h0((5 if a < 14 else 14), total(build(4, b))))
    return h0(12, h0(a, x))


def h2(a, b):
    x = (h1(total(build(5, b)), 8) if 1 < h0((b if b < 1 else 1), pair(a, b)[0]) else (ap(lambda x: x * a + 0, b) if ap(lambda x: x * 3 + 0, b) < total(build(5, b)) else (a + 8)))
    return x


def main():
    y = ap(lambda x: x * 9 + 5, ap(lambda x: x * pair(8, 18)[1] + 3, total(build(4, 19))))
    f = lambda x: x * ((y ^ 9) if 9 < 18 else h2(19, 10)) + y
    l = build(0, y)
    return (h0(h2(ap(lambda x: x * 8 + 3, y), (y - 18)), h1((4 if y < y else 10), (y ^ 16))), y, f(h0(h2(ap(lambda x: x * 8 + 3, y), (y - 18)), h1((4 if y < y else 10), (y ^ 16)))) + f(y), total(l) + total(l))
