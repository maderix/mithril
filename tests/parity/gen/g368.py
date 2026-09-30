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
    x = ap(lambda x: x * 8 + 2, b)
    return ((total(build(6, 15)) * (15 & b)) if total(build(3, (12 + 5))) < 19 else (pair(x, x)[0] if b < (a ^ a) else pair(11, 12)[1]))


def h1(a, b):
    x = pair((14 ^ (b if b < a else a)), pair(total(build(5, b)), h0(a, 2))[1])[0]
    return 1


def h2(a, b):
    x = ap(lambda x: x * ap(lambda x: x * 13 + 2, total(build(1, b))) + 0, pair(b, 1)[1])
    return ap(lambda x: x * ((6 if a < a else b) & total(build(0, 10))) + 5, 9)


def main():
    y = ap(lambda x: x * 0 + 8, total(build(3, h2(17, 10))))
    f = lambda x: x * total(build(6, (3 if 15 < y else 0))) + y
    l = build(4, y)
    return (y, y, f(y) + f(y), total(l) + total(l))
