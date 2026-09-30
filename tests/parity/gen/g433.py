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
    x = ((pair(b, 6)[1] + ap(lambda x: x * 18 + 0, a)) + pair((b if b < 11 else a), (b * a))[1])
    return total(build(1, ap(lambda x: x * pair(x, x)[0] + 4, b)))


def h1(a, b):
    x = pair(a, (4 if 15 < pair(b, a)[0] else h0(a, a)))[0]
    return total(build(3, (a if total(build(2, 18)) < (3 + x) else 19)))


def h2(a, b):
    x = ap(lambda x: x * a + 5, ap(lambda x: x * 17 + 3, pair(b, b)[1]))
    return total(build(4, h1(pair(a, x)[1], total(build(1, x)))))


def main():
    y = 3
    f = lambda x: x * (h2(14, y) if 0 < total(build(1, 5)) else 18) + y
    l = build(1, y)
    return (total(build(1, ap(lambda x: x * pair(y, 11)[0] + 6, (4 ^ y)))), ap(lambda x: x * h1(total(build(5, y)), pair(y, 19)[0]) + 5, y), f(total(build(1, ap(lambda x: x * pair(y, 11)[0] + 6, (4 ^ y))))) + f(y), total(l) + total(l))
