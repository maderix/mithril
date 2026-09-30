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
    x = ap(lambda x: x * pair((12 + 7), ap(lambda x: x * a + 5, 13))[0] + 8, ((18 + b) + (2 ^ a)))
    return 4


def h1(a, b):
    x = h0(total(build(3, total(build(3, b)))), total(build(6, pair(b, a)[1])))
    return total(build(6, total(build(2, (2 ^ 16)))))


def h2(a, b):
    x = h1(ap(lambda x: x * h0(16, 19) + 4, a), 17)
    return (total(build(6, pair(b, x)[0])) if ap(lambda x: x * h1(19, b) + 1, total(build(1, b))) < h1(9, h0(x, 5)) else total(build(5, pair(6, 1)[1])))


def main():
    y = h1(h2(h2(0, 0), ap(lambda x: x * 19 + 3, 10)), ap(lambda x: x * (16 if 16 < 16 else 5) + 8, 16))
    f = lambda x: x * 5 + y
    l = build(3, y)
    return ((pair(16, total(build(3, 15)))[1] & ((y if y < y else 7) ^ total(build(3, y)))), 7, f((pair(16, total(build(3, 15)))[1] & ((y if y < y else 7) ^ total(build(3, y))))) + f(y), total(l) + total(l))
