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
    x = (total(build(1, 3)) if total(build(6, b)) < pair(total(build(4, 16)), total(build(0, a)))[0] else total(build(6, pair(a, 16)[0])))
    return ap(lambda x: x * pair(19, (14 if 19 < a else b))[0] + 5, total(build(1, b)))


def h1(a, b):
    x = total(build(6, a))
    return pair((2 if h0(a, 15) < ap(lambda x: x * x + 0, 8) else h0(b, 8)), h0((3 if x < 17 else a), total(build(0, 7))))[1]


def h2(a, b):
    x = (((b ^ b) if 4 < total(build(6, 19)) else total(build(4, 19))) + 12)
    return h1(total(build(4, (b * b))), (ap(lambda x: x * b + 7, a) if 4 < b else total(build(5, 1))))


def main():
    y = 7
    f = lambda x: x * (h0(y, 0) if h1(4, y) < (3 if 17 < 19 else y) else pair(y, y)[0]) + y
    l = build(4, y)
    return (h1(pair(total(build(4, 6)), total(build(4, y)))[0], total(build(2, pair(3, 15)[1]))), ((4 if (y - 7) < (y + 7) else h1(19, 0)) if (pair(3, y)[1] if h2(y, 8) < pair(y, y)[1] else 16) < ((0 if y < 9 else y) - pair(y, 1)[0]) else (1 + 12)), f(h1(pair(total(build(4, 6)), total(build(4, y)))[0], total(build(2, pair(3, 15)[1])))) + f(y), total(l) + total(l))
