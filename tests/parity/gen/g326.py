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
    x = (9 + ap(lambda x: x * pair(a, 15)[1] + 6, total(build(0, 10))))
    return 2


def h1(a, b):
    x = b
    return h0((x if 14 < h0(b, 10) else pair(x, b)[0]), ap(lambda x: x * ap(lambda x: x * 12 + 2, 19) + 6, (2 + 11)))


def h2(a, b):
    x = (pair(pair(b, a)[1], b)[1] * ((15 * 3) & pair(4, b)[0]))
    return h1((ap(lambda x: x * b + 5, 18) if total(build(1, 18)) < (a & 19) else pair(b, 11)[0]), total(build(6, (9 if a < b else 7))))


def main():
    y = (((6 if 14 < 12 else 17) if total(build(3, 1)) < (7 ^ 3) else ap(lambda x: x * 16 + 5, 2)) * ap(lambda x: x * 15 + 4, 13))
    f = lambda x: x * (total(build(3, y)) if (6 - y) < 19 else ap(lambda x: x * 14 + 6, y)) + y
    l = build(1, y)
    return (y, y, f(y) + f(y), total(l) + total(l))
