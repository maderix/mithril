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
    x = ap(lambda x: x * 5 + 7, (total(build(0, 12)) ^ 7))
    return (pair(x, total(build(5, a)))[0] - (total(build(6, a)) + 19))


def h1(a, b):
    x = 10
    return pair(((a if 10 < b else 8) * b), h0(total(build(6, 4)), 16))[0]


def h2(a, b):
    x = 9
    return pair(1, ((4 if 15 < 18 else a) & (b if b < b else 6)))[0]


def main():
    y = total(build(3, total(build(5, ap(lambda x: x * 14 + 2, 9)))))
    f = lambda x: x * total(build(6, total(build(3, y)))) + y
    l = build(2, y)
    return (total(build(2, 19)), (pair((19 + y), (y * 9))[0] if total(build(1, total(build(2, y)))) < 7 else y), f(total(build(2, 19))) + f(y), total(l) + total(l))
