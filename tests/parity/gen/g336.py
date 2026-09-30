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
    x = total(build(2, ((17 if 12 < 9 else 9) if 3 < (b if 3 < 19 else 13) else ap(lambda x: x * b + 6, b))))
    return (total(build(4, (14 & 7))) - total(build(6, ap(lambda x: x * b + 7, 19))))


def h1(a, b):
    x = total(build(3, 5))
    return h0((ap(lambda x: x * 2 + 2, x) if (0 if a < 10 else a) < ap(lambda x: x * x + 5, x) else 10), ap(lambda x: x * h0(x, 5) + 0, total(build(6, 18))))


def h2(a, b):
    x = pair((12 & ap(lambda x: x * 5 + 3, b)), h0(b, 1))[0]
    return total(build(6, pair((x if x < 10 else 19), (19 & 1))[0]))


def main():
    y = h2(total(build(5, 7)), (total(build(2, 5)) if h0(5, 8) < pair(9, 14)[0] else ap(lambda x: x * 13 + 7, 0)))
    f = lambda x: x * (6 if 5 < (y if 6 < 1 else 10) else pair(6, 8)[1]) + y
    l = build(2, y)
    return (pair(((2 & 10) if y < 5 else ap(lambda x: x * 1 + 8, 13)), y)[0], (pair(total(build(1, 2)), pair(y, y)[0])[1] if 9 < 1 else ap(lambda x: x * (16 + 0) + 1, (y ^ 12))), f(pair(((2 & 10) if y < 5 else ap(lambda x: x * 1 + 8, 13)), y)[0]) + f(y), total(l) + total(l))
