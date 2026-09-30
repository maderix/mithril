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
    x = ((pair(a, 14)[1] & (b if 9 < b else a)) if (18 if (10 if b < a else 9) < (1 * 18) else (18 + 1)) < 12 else a)
    return total(build(1, total(build(3, b))))


def h1(a, b):
    x = b
    return pair(0, h0((17 if a < 0 else 5), h0(b, 0)))[0]


def h2(a, b):
    x = pair(ap(lambda x: x * ap(lambda x: x * a + 0, b) + 3, total(build(4, 11))), total(build(6, total(build(2, a)))))[0]
    return x


def main():
    y = 10
    f = lambda x: x * 16 + y
    l = build(3, y)
    return (4, h0(ap(lambda x: x * 0 + 7, (3 if 2 < 13 else y)), (ap(lambda x: x * 9 + 0, 4) ^ total(build(4, y)))), f(4) + f(y), total(l) + total(l))
