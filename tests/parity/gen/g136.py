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
    x = pair((15 ^ (b if b < 14 else a)), ((8 if 3 < 18 else b) * pair(a, 6)[0]))[1]
    return (a if (b if b < x else (2 if 7 < 12 else x)) < total(build(1, ap(lambda x: x * b + 6, 15))) else (pair(4, b)[0] if (1 ^ a) < (16 - b) else (0 - 0)))


def h1(a, b):
    x = pair((total(build(5, 15)) if (19 * 7) < ap(lambda x: x * 5 + 3, b) else b), (ap(lambda x: x * 19 + 0, a) & a))[0]
    return ((ap(lambda x: x * 0 + 7, b) if a < ap(lambda x: x * 1 + 6, a) else 18) + a)


def h2(a, b):
    x = h1((pair(15, 1)[0] if total(build(0, b)) < total(build(0, 18)) else (b if 17 < a else b)), h1(h1(5, 17), 11))
    return a


def main():
    y = (((15 if 3 < 11 else 9) if (7 if 17 < 2 else 6) < total(build(3, 2)) else (3 + 19)) * (h0(0, 8) - 3))
    f = lambda x: x * total(build(2, (17 & 5))) + y
    l = build(4, y)
    return (pair(h0(y, h2(y, 10)), 15)[0], h2(((16 if 2 < y else 9) if 19 < total(build(2, y)) else 11), y), f(pair(h0(y, h2(y, 10)), 15)[0]) + f(y), total(l) + total(l))
