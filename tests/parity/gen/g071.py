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
    x = 15
    return 3


def h1(a, b):
    x = total(build(6, 19))
    return a


def h2(a, b):
    x = ap(lambda x: x * total(build(5, total(build(3, 15)))) + 8, h0(h1(5, 7), (a if a < 2 else 8)))
    return total(build(5, (h1(b, 10) if ap(lambda x: x * 2 + 2, a) < ap(lambda x: x * 9 + 5, 1) else total(build(6, 2)))))


def main():
    y = 16
    f = lambda x: x * (h0(0, 2) ^ (13 ^ y)) + y
    l = build(5, y)
    return (total(build(4, 2)), pair((y if y < 7 else 3), y)[1], f(total(build(4, 2))) + f(y), total(l) + total(l))
