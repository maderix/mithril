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
    x = (6 if 3 < 7 else 10)
    return pair(total(build(4, 13)), 16)[1]


def h1(a, b):
    x = pair(ap(lambda x: x * (11 if b < 12 else a) + 1, 5), total(build(1, b)))[0]
    return a


def h2(a, b):
    x = total(build(1, a))
    return ((pair(17, 9)[1] & 10) if b < h1(h1(b, 11), a) else ((14 + 3) + (19 if 1 < 0 else 18)))


def main():
    y = (((1 if 19 < 9 else 15) - ap(lambda x: x * 7 + 4, 3)) + total(build(2, ap(lambda x: x * 5 + 8, 19))))
    f = lambda x: x * pair((y if 16 < 5 else 8), y)[1] + y
    l = build(5, y)
    return ((15 - total(build(0, y))), ap(lambda x: x * ap(lambda x: x * (15 - 17) + 2, (y ^ 18)) + 3, y), f((15 - total(build(0, y)))) + f(y), total(l) + total(l))
