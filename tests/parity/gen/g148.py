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
    x = ap(lambda x: x * b + 1, ap(lambda x: x * a + 0, pair(a, 10)[1]))
    return 11


def h1(a, b):
    x = pair(h0(5, b), total(build(0, a)))[1]
    return x


def h2(a, b):
    x = (((16 if b < 3 else 10) + total(build(2, b))) if pair(pair(a, b)[0], b)[1] < pair(h0(4, 19), total(build(6, b)))[0] else (h0(b, a) ^ (b ^ 9)))
    return ap(lambda x: x * ((b if 0 < a else x) if (16 & 9) < ap(lambda x: x * a + 6, 4) else h1(a, x)) + 4, h0(a, h1(0, 13)))


def main():
    y = ap(lambda x: x * (total(build(5, 6)) + (0 + 6)) + 6, ap(lambda x: x * pair(9, 19)[1] + 1, (15 if 12 < 1 else 14)))
    f = lambda x: x * h1(pair(y, y)[0], total(build(5, 7))) + y
    l = build(1, y)
    return (total(build(6, total(build(3, h1(y, y))))), 6, f(total(build(6, total(build(3, h1(y, y)))))) + f(y), total(l) + total(l))
