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
    x = (ap(lambda x: x * (a if 4 < b else 0) + 2, total(build(2, b))) if 4 < ap(lambda x: x * total(build(5, a)) + 4, 4) else ap(lambda x: x * ap(lambda x: x * a + 8, 14) + 1, 2))
    return total(build(2, 14))


def h1(a, b):
    x = h0(total(build(4, pair(12, 7)[1])), total(build(2, pair(b, a)[0])))
    return (pair((18 * 17), ap(lambda x: x * 15 + 6, 8))[1] & h0(x, pair(b, 1)[0]))


def h2(a, b):
    x = (total(build(0, total(build(3, 7)))) if total(build(2, total(build(3, a)))) < 5 else (b - b))
    return 14


def main():
    y = 9
    f = lambda x: x * y + y
    l = build(4, y)
    return (h2(ap(lambda x: x * y + 4, (16 ^ 17)), h1(ap(lambda x: x * y + 3, y), pair(9, 16)[1])), total(build(2, h2(total(build(5, y)), y))), f(h2(ap(lambda x: x * y + 4, (16 ^ 17)), h1(ap(lambda x: x * y + 3, y), pair(9, 16)[1]))) + f(y), total(l) + total(l))
