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
    x = a
    return a


def h1(a, b):
    x = a
    return pair((h0(x, 1) if ap(lambda x: x * b + 6, 6) < 19 else (5 if a < b else x)), ap(lambda x: x * ap(lambda x: x * b + 0, b) + 1, b))[0]


def h2(a, b):
    x = b
    return pair(ap(lambda x: x * (7 * a) + 6, 8), x)[0]


def main():
    y = ap(lambda x: x * 15 + 0, ((8 & 10) + (6 - 6)))
    f = lambda x: x * ((11 + y) if (y if y < y else 3) < pair(4, 6)[0] else total(build(3, 5))) + y
    l = build(1, y)
    return (total(build(6, (h1(y, 9) if (13 if 17 < y else y) < pair(9, 7)[0] else h1(19, y)))), (y ^ h1((14 if y < y else 13), ap(lambda x: x * 2 + 7, 8))), f(total(build(6, (h1(y, 9) if (13 if 17 < y else y) < pair(9, 7)[0] else h1(19, y))))) + f(y), total(l) + total(l))
