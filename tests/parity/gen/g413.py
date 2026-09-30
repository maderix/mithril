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
    x = ((pair(13, a)[0] if 18 < ap(lambda x: x * a + 1, a) else (b + 13)) & (b if 18 < 15 else ap(lambda x: x * 11 + 2, 7)))
    return b


def h1(a, b):
    x = (7 if (ap(lambda x: x * 11 + 1, b) + b) < b else h0((b if a < a else 12), ap(lambda x: x * 5 + 0, 1)))
    return pair((h0(x, x) & 13), h0(total(build(4, a)), h0(7, 7)))[1]


def h2(a, b):
    x = a
    return h0(h1(total(build(5, b)), (b + 18)), 10)


def main():
    y = h2(9, h2(1, (6 ^ 17)))
    f = lambda x: x * y + y
    l = build(1, y)
    return (ap(lambda x: x * ap(lambda x: x * 10 + 1, ap(lambda x: x * 3 + 0, y)) + 2, h0((1 if y < y else y), pair(y, y)[0])), h2(pair((1 ^ y), total(build(5, y)))[1], ap(lambda x: x * 4 + 8, y)), f(ap(lambda x: x * ap(lambda x: x * 10 + 1, ap(lambda x: x * 3 + 0, y)) + 2, h0((1 if y < y else y), pair(y, y)[0]))) + f(y), total(l) + total(l))
