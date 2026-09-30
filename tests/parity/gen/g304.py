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
    x = 0
    return total(build(4, (pair(b, 11)[0] ^ a)))


def h1(a, b):
    x = (h0(ap(lambda x: x * a + 0, b), (a & 5)) + (h0(17, b) & ap(lambda x: x * 1 + 3, 15)))
    return total(build(2, ap(lambda x: x * (13 if b < a else 6) + 8, h0(13, 3))))


def h2(a, b):
    x = total(build(6, ((b if 19 < b else a) ^ ap(lambda x: x * b + 7, 10))))
    return ((11 if (x ^ 9) < (4 if 5 < x else b) else h1(b, 3)) if pair(total(build(1, a)), pair(b, 16)[1])[0] < a else (h1(8, 13) if (x * b) < (x if a < b else x) else ap(lambda x: x * x + 7, a)))


def main():
    y = 16
    f = lambda x: x * total(build(4, ap(lambda x: x * 16 + 2, 14))) + y
    l = build(1, y)
    return (0, total(build(4, ((18 if 18 < y else 12) * (y if 13 < y else y)))), f(0) + f(y), total(l) + total(l))
