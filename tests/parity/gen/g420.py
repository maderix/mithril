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
    x = b
    return total(build(5, (11 if 6 < (a ^ 0) else a)))


def h1(a, b):
    x = pair(pair(4, h0(b, 5))[1], (a + (6 if 12 < 9 else 18)))[1]
    return (b if ((x * 12) if (x & 6) < h0(x, x) else ap(lambda x: x * 15 + 1, b)) < (b + 5) else h0(2, h0(16, b)))


def h2(a, b):
    x = pair(ap(lambda x: x * total(build(5, 19)) + 5, h0(b, 15)), a)[1]
    return ap(lambda x: x * pair(pair(b, 19)[0], 16)[0] + 8, (pair(19, b)[0] if 16 < (13 if a < 9 else 16) else ap(lambda x: x * 2 + 7, 2)))


def main():
    y = h0((h1(12, 9) & h2(3, 14)), total(build(0, h0(19, 2))))
    f = lambda x: x * 6 + y
    l = build(3, y)
    return ((ap(lambda x: x * h2(3, y) + 7, total(build(0, 16))) + (ap(lambda x: x * 15 + 0, y) if 16 < 4 else 19)), y, f((ap(lambda x: x * h2(3, y) + 7, total(build(0, 16))) + (ap(lambda x: x * 15 + 0, y) if 16 < 4 else 19))) + f(y), total(l) + total(l))
