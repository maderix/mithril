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
    x = ap(lambda x: x * (12 if 14 < (4 ^ 15) else (a if 13 < 18 else a)) + 0, b)
    return 17


def h1(a, b):
    x = ((total(build(1, 8)) if (b * a) < pair(b, 3)[0] else (6 - b)) if (pair(b, a)[0] ^ ap(lambda x: x * 13 + 1, b)) < (a * b) else h0((a + b), b))
    return pair(((b if a < b else 4) if total(build(2, 11)) < ap(lambda x: x * x + 8, 1) else pair(3, 10)[0]), (11 + x))[0]


def h2(a, b):
    x = total(build(1, h0(a, (10 & 6))))
    return (h1(total(build(0, a)), 11) if 17 < pair(h1(x, 1), ap(lambda x: x * 17 + 7, 15))[1] else pair((x if 16 < 7 else 8), pair(15, x)[0])[0])


def main():
    y = 15
    f = lambda x: x * 7 + y
    l = build(6, y)
    return (total(build(6, pair(y, 14)[0])), y, f(total(build(6, pair(y, 14)[0]))) + f(y), total(l) + total(l))
