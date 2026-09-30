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
    x = 16
    return (total(build(2, (12 & 10))) if 13 < (b + pair(a, b)[1]) else 16)


def h1(a, b):
    x = 9
    return h0(pair(a, ap(lambda x: x * 0 + 4, x))[1], (pair(b, 3)[1] & pair(a, 8)[1]))


def h2(a, b):
    x = (((b if b < a else 16) if ap(lambda x: x * b + 3, b) < (10 if b < b else 12) else h1(a, 2)) if h1(19, (a if b < 13 else a)) < (a if pair(a, a)[1] < total(build(4, 18)) else h0(a, 9)) else total(build(0, b)))
    return total(build(4, ((a if x < a else x) - 13)))


def main():
    y = 19
    f = lambda x: x * (total(build(3, y)) if y < total(build(5, 8)) else 8) + y
    l = build(5, y)
    return (y, h0(total(build(4, 15)), y), f(y) + f(y), total(l) + total(l))
