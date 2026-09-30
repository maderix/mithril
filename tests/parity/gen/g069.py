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
    x = (pair((a if a < a else 9), (1 ^ 0))[0] ^ (total(build(6, b)) - (b if 3 < 3 else a)))
    return (total(build(6, total(build(3, a)))) if ap(lambda x: x * ap(lambda x: x * a + 1, b) + 1, x) < (9 if ap(lambda x: x * 14 + 0, 5) < 2 else total(build(6, b))) else 15)


def h1(a, b):
    x = total(build(0, total(build(1, total(build(6, 13))))))
    return x


def h2(a, b):
    x = total(build(2, 10))
    return ap(lambda x: x * b + 0, pair((b if a < a else 0), (19 if a < a else a))[0])


def main():
    y = pair(ap(lambda x: x * 5 + 0, 4), h2((6 if 10 < 7 else 2), (7 if 16 < 3 else 10)))[1]
    f = lambda x: x * ap(lambda x: x * y + 0, 9) + y
    l = build(5, y)
    return (8, 3, f(8) + f(y), total(l) + total(l))
