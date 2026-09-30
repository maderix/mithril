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
    x = 6
    return total(build(5, ((15 - 1) if ap(lambda x: x * x + 5, 8) < total(build(6, a)) else total(build(6, 2)))))


def h1(a, b):
    x = total(build(6, (15 if (b if b < b else b) < total(build(4, 3)) else (9 ^ b))))
    return pair(total(build(2, pair(19, b)[1])), 17)[0]


def h2(a, b):
    x = a
    return total(build(2, (total(build(1, x)) if x < 10 else pair(a, a)[0])))


def main():
    y = 10
    f = lambda x: x * ap(lambda x: x * 7 + 4, ap(lambda x: x * y + 7, 17)) + y
    l = build(1, y)
    return (total(build(2, total(build(0, total(build(5, y)))))), (pair(y, (y ^ 1))[0] if (total(build(2, 10)) if h2(y, 16) < (y if 3 < y else y) else ap(lambda x: x * 16 + 8, 8)) < 9 else ap(lambda x: x * y + 1, total(build(6, y)))), f(total(build(2, total(build(0, total(build(5, y))))))) + f(y), total(l) + total(l))
