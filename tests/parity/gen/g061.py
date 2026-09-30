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
    x = (pair(17, total(build(1, a)))[1] if pair(19, pair(b, 15)[1])[1] < 11 else (total(build(3, b)) if 9 < 9 else 19))
    return pair(total(build(4, total(build(0, x)))), ap(lambda x: x * total(build(2, b)) + 4, ap(lambda x: x * a + 1, b)))[1]


def h1(a, b):
    x = total(build(4, total(build(3, (a & 9)))))
    return a


def h2(a, b):
    x = (ap(lambda x: x * (b - a) + 0, (8 & b)) if (ap(lambda x: x * a + 1, 4) + (18 - b)) < ((0 ^ a) + h0(a, a)) else h1(ap(lambda x: x * 1 + 4, 14), pair(b, 2)[0]))
    return pair(b, pair(h1(x, 0), ap(lambda x: x * 19 + 5, b))[1])[0]


def main():
    y = 16
    f = lambda x: x * y + y
    l = build(0, y)
    return (total(build(6, pair((15 ^ y), (16 + 16))[0])), (ap(lambda x: x * total(build(4, y)) + 7, total(build(6, 16))) * ap(lambda x: x * total(build(4, y)) + 7, (y if 5 < y else 19))), f(total(build(6, pair((15 ^ y), (16 + 16))[0]))) + f(y), total(l) + total(l))
