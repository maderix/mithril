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
    x = pair(ap(lambda x: x * 12 + 0, total(build(1, 3))), ((b if b < 14 else b) if (b + b) < ap(lambda x: x * b + 1, a) else (14 + b)))[1]
    return pair((9 * ap(lambda x: x * a + 4, b)), 19)[1]


def h1(a, b):
    x = pair(pair(a, pair(5, b)[1])[0], (a if 8 < a else total(build(4, 17))))[1]
    return total(build(3, ap(lambda x: x * pair(b, 7)[1] + 8, 19)))


def h2(a, b):
    x = b
    return ap(lambda x: x * ap(lambda x: x * a + 3, (11 ^ 5)) + 4, (total(build(0, 18)) if (b ^ 14) < h1(4, x) else 17))


def main():
    y = pair(ap(lambda x: x * 2 + 5, h2(18, 15)), (ap(lambda x: x * 18 + 7, 17) + pair(9, 8)[0]))[1]
    f = lambda x: x * 18 + y
    l = build(6, y)
    return ((4 if h0(y, ap(lambda x: x * 11 + 6, y)) < total(build(6, h2(y, y))) else 11), (18 if y < y else (pair(y, 13)[1] if (1 if y < y else y) < (y if 12 < 9 else 2) else 4)), f((4 if h0(y, ap(lambda x: x * 11 + 6, y)) < total(build(6, h2(y, y))) else 11)) + f(y), total(l) + total(l))
