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
    x = b
    return 16


def h1(a, b):
    x = a
    return total(build(3, 13))


def h2(a, b):
    x = pair((total(build(0, a)) + total(build(2, 9))), h0(ap(lambda x: x * 13 + 5, b), 17))[0]
    return 16


def main():
    y = (total(build(1, total(build(4, 15)))) if ((18 * 6) ^ pair(9, 11)[0]) < ap(lambda x: x * h2(4, 1) + 5, 17) else 19)
    f = lambda x: x * 3 + y
    l = build(4, y)
    return (0, (ap(lambda x: x * y + 7, (y * 18)) + ap(lambda x: x * ap(lambda x: x * 4 + 3, 7) + 1, 17)), f(0) + f(y), total(l) + total(l))
