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
    x = total(build(6, (total(build(1, b)) - total(build(0, 2)))))
    return ((b + (18 ^ x)) if pair((b + 9), x)[0] < ((12 * 19) + 7) else a)


def h1(a, b):
    x = h0(h0(h0(11, 9), total(build(2, 13))), 2)
    return h0(((4 & a) if ap(lambda x: x * b + 1, 6) < h0(b, x) else a), pair((b if x < x else 1), pair(x, 13)[1])[1])


def h2(a, b):
    x = b
    return ap(lambda x: x * pair(3, total(build(1, 17)))[0] + 2, pair((9 if 19 < b else 14), pair(17, 15)[1])[1])


def main():
    y = (ap(lambda x: x * 5 + 6, ap(lambda x: x * 9 + 4, 19)) if (9 + pair(13, 6)[0]) < 1 else (h0(0, 10) if h0(18, 8) < h2(7, 19) else 3))
    f = lambda x: x * h0((0 * y), pair(17, y)[0]) + y
    l = build(6, y)
    return (total(build(1, ap(lambda x: x * pair(15, 19)[1] + 8, (4 if 5 < 8 else 9)))), h0((pair(1, 0)[1] if y < h2(y, 19) else 2), (11 if 2 < y else (18 * y))), f(total(build(1, ap(lambda x: x * pair(15, 19)[1] + 8, (4 if 5 < 8 else 9))))) + f(y), total(l) + total(l))
