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
    x = pair(0, 19)[0]
    return (ap(lambda x: x * 15 + 0, pair(5, 12)[0]) if ap(lambda x: x * ap(lambda x: x * 12 + 3, 3) + 1, pair(a, b)[1]) < b else a)


def h1(a, b):
    x = ((pair(12, a)[1] & a) if 19 < h0(total(build(6, a)), (17 - 18)) else total(build(2, (a ^ 6))))
    return ap(lambda x: x * (7 * (b if 5 < x else 6)) + 8, h0((12 if b < 13 else 12), pair(6, 12)[1]))


def h2(a, b):
    x = total(build(0, ap(lambda x: x * (a if 0 < a else b) + 1, pair(2, 0)[0])))
    return ((13 if pair(b, a)[1] < h0(12, 17) else 15) if total(build(6, (6 if 18 < b else a))) < (ap(lambda x: x * x + 7, 2) * h0(b, 15)) else pair((6 if 14 < 6 else 12), 4)[0])


def main():
    y = 0
    f = lambda x: x * pair(14, pair(8, 4)[1])[0] + y
    l = build(6, y)
    return ((ap(lambda x: x * total(build(6, 1)) + 1, h2(5, 8)) * total(build(6, (y if 2 < 3 else 9)))), ((9 & (19 - y)) & ((11 - 15) if y < (9 * 4) else ap(lambda x: x * 7 + 6, y))), f((ap(lambda x: x * total(build(6, 1)) + 1, h2(5, 8)) * total(build(6, (y if 2 < 3 else 9))))) + f(y), total(l) + total(l))
