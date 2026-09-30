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
    x = ((b & (10 if 4 < 9 else b)) if (total(build(3, 18)) if 5 < total(build(3, 17)) else total(build(4, 17))) < total(build(1, 4)) else 18)
    return ((ap(lambda x: x * 13 + 6, b) if 16 < pair(1, 18)[0] else 15) if pair(b, a)[0] < (6 - 16) else ((11 if 15 < b else x) if 8 < pair(7, 13)[1] else x))


def h1(a, b):
    x = total(build(6, (total(build(4, b)) ^ b)))
    return h0(total(build(1, total(build(0, b)))), (pair(8, 10)[1] if pair(x, x)[1] < 10 else b))


def h2(a, b):
    x = ap(lambda x: x * pair(5, 18)[1] + 2, ap(lambda x: x * b + 7, ap(lambda x: x * a + 4, a)))
    return pair(16, a)[1]


def main():
    y = 13
    f = lambda x: x * 17 + y
    l = build(2, y)
    return (13, y, f(13) + f(y), total(l) + total(l))
