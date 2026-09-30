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
    x = pair(15, ap(lambda x: x * total(build(5, 4)) + 6, 5))[0]
    return (16 * 14)


def h1(a, b):
    x = h0((b + a), ap(lambda x: x * h0(b, a) + 6, ap(lambda x: x * b + 3, 0)))
    return ap(lambda x: x * 17 + 3, total(build(1, h0(b, x))))


def h2(a, b):
    x = (pair(h0(3, b), (a - b))[1] if b < ap(lambda x: x * total(build(5, a)) + 0, (b if b < 15 else b)) else h0(h1(10, 12), 11))
    return (((19 - x) * h1(16, 11)) ^ x)


def main():
    y = (total(build(0, 18)) - (ap(lambda x: x * 7 + 0, 9) if 7 < h1(6, 8) else pair(12, 2)[0]))
    f = lambda x: x * pair(h2(4, y), pair(1, y)[1])[0] + y
    l = build(4, y)
    return (ap(lambda x: x * h1((y & 7), (5 if 6 < y else y)) + 8, 7), h2(h0(total(build(0, y)), total(build(1, y))), y), f(ap(lambda x: x * h1((y & 7), (5 if 6 < y else y)) + 8, 7)) + f(y), total(l) + total(l))
