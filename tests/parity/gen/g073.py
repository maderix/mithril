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
    x = total(build(6, (total(build(1, 18)) & pair(a, a)[1])))
    return total(build(1, 0))


def h1(a, b):
    x = (ap(lambda x: x * h0(a, 14) + 2, total(build(6, 8))) if a < total(build(2, ap(lambda x: x * 3 + 8, a))) else pair(b, (2 + 17))[1])
    return total(build(0, (ap(lambda x: x * 2 + 5, x) - total(build(2, 2)))))


def h2(a, b):
    x = b
    return total(build(5, b))


def main():
    y = (0 if h1(h1(5, 16), 18) < ap(lambda x: x * (8 ^ 12) + 3, total(build(3, 15))) else ap(lambda x: x * (5 if 5 < 7 else 1) + 1, ap(lambda x: x * 9 + 0, 19)))
    f = lambda x: x * 0 + y
    l = build(3, y)
    return ((y & 8), ap(lambda x: x * total(build(1, ap(lambda x: x * 13 + 2, 14))) + 6, (h1(9, 6) if 13 < (y if 9 < y else 6) else 2)), f((y & 8)) + f(y), total(l) + total(l))
