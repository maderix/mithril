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
    x = ((ap(lambda x: x * b + 2, 17) if (a - a) < ap(lambda x: x * a + 3, a) else a) if 5 < (a if 5 < 1 else 5) else pair(14, b)[1])
    return 14


def h1(a, b):
    x = h0((a if ap(lambda x: x * b + 3, b) < ap(lambda x: x * b + 4, 2) else (11 if a < b else 8)), total(build(0, (14 - 1))))
    return total(build(5, h0(b, total(build(2, x)))))


def h2(a, b):
    x = ap(lambda x: x * total(build(2, (16 if a < b else 17))) + 2, total(build(6, a)))
    return pair(total(build(3, x)), (h0(a, 12) - 17))[1]


def main():
    y = h2(total(build(6, total(build(1, 14)))), 3)
    f = lambda x: x * (pair(y, 1)[0] if y < h2(0, y) else ap(lambda x: x * y + 3, 13)) + y
    l = build(3, y)
    return (h2(y, y), ((1 * (y - y)) if 16 < y else (h1(11, y) * (y if y < 17 else y))), f(h2(y, y)) + f(y), total(l) + total(l))
