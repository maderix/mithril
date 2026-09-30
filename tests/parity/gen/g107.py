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
    x = total(build(0, 4))
    return 4


def h1(a, b):
    x = (ap(lambda x: x * (a if 4 < b else 7) + 2, h0(a, b)) if 2 < ap(lambda x: x * a + 4, (a if a < 10 else 8)) else (ap(lambda x: x * 18 + 1, b) if h0(b, 7) < (16 - a) else (a & b)))
    return a


def h2(a, b):
    x = b
    return pair(total(build(0, 9)), pair((x + 16), b)[0])[1]


def main():
    y = h1(total(build(6, pair(11, 17)[0])), (17 if 1 < h2(14, 7) else total(build(3, 18))))
    f = lambda x: x * pair(pair(y, y)[1], y)[0] + y
    l = build(1, y)
    return (total(build(1, ((12 ^ y) if pair(2, y)[0] < y else ap(lambda x: x * 3 + 2, y)))), (1 & y), f(total(build(1, ((12 ^ y) if pair(2, y)[0] < y else ap(lambda x: x * 3 + 2, y))))) + f(y), total(l) + total(l))
