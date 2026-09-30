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
    x = total(build(5, pair(16, b)[0]))
    return 10


def h1(a, b):
    x = pair(ap(lambda x: x * (12 ^ a) + 2, b), h0((11 + 14), total(build(2, 7))))[1]
    return pair((pair(11, 10)[1] if total(build(6, x)) < pair(a, b)[1] else 14), pair(7, ap(lambda x: x * 4 + 4, 15))[0])[1]


def h2(a, b):
    x = (h0(a, (a if a < b else b)) & total(build(6, (a ^ a))))
    return h0(h1((x - b), ap(lambda x: x * 11 + 6, 18)), x)


def main():
    y = 6
    f = lambda x: x * ap(lambda x: x * h0(y, y) + 3, pair(y, y)[1]) + y
    l = build(4, y)
    return (h1(y, ap(lambda x: x * total(build(4, 11)) + 7, 7)), total(build(1, (y ^ y))), f(h1(y, ap(lambda x: x * total(build(4, 11)) + 7, 7))) + f(y), total(l) + total(l))
