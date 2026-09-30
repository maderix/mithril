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
    x = ap(lambda x: x * total(build(1, total(build(2, b)))) + 5, 11)
    return ap(lambda x: x * (total(build(4, x)) ^ pair(x, x)[0]) + 0, 19)


def h1(a, b):
    x = a
    return 11


def h2(a, b):
    x = ap(lambda x: x * h0((a if a < 7 else b), h1(a, b)) + 0, (pair(18, a)[0] if (b - a) < a else total(build(1, b))))
    return total(build(6, 18))


def main():
    y = total(build(1, pair(7, 8)[0]))
    f = lambda x: x * ap(lambda x: x * pair(y, y)[0] + 8, pair(y, 19)[0]) + y
    l = build(2, y)
    return (h2(total(build(4, y)), pair(pair(18, y)[1], total(build(6, y)))[1]), h1(pair((y * y), ap(lambda x: x * y + 8, y))[1], 10), f(h2(total(build(4, y)), pair(pair(18, y)[1], total(build(6, y)))[1])) + f(y), total(l) + total(l))
