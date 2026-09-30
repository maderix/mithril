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
    x = total(build(4, ap(lambda x: x * total(build(6, b)) + 7, (a if 2 < b else 6))))
    return a


def h1(a, b):
    x = ((a - total(build(4, 12))) * a)
    return h0((x & (b if a < a else a)), ap(lambda x: x * (1 & 11) + 4, pair(x, x)[1]))


def h2(a, b):
    x = (h1(pair(a, b)[1], total(build(1, b))) if ap(lambda x: x * total(build(3, 10)) + 5, ap(lambda x: x * a + 1, 10)) < pair((b if 17 < 10 else 11), h0(b, 13))[0] else (h1(11, 19) if 4 < 19 else a))
    return ap(lambda x: x * a + 3, pair(7, pair(a, 5)[1])[0])


def main():
    y = total(build(5, total(build(4, total(build(1, 0))))))
    f = lambda x: x * 6 + y
    l = build(0, y)
    return (ap(lambda x: x * total(build(2, (19 if 18 < 12 else 19))) + 4, (ap(lambda x: x * 7 + 6, 14) * 2)), y, f(ap(lambda x: x * total(build(2, (19 if 18 < 12 else 19))) + 4, (ap(lambda x: x * 7 + 6, 14) * 2))) + f(y), total(l) + total(l))
