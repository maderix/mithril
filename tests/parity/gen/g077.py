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
    x = ap(lambda x: x * 15 + 6, total(build(2, (b + 12))))
    return (total(build(5, x)) * ap(lambda x: x * ap(lambda x: x * b + 5, x) + 6, (x if b < x else 8)))


def h1(a, b):
    x = total(build(5, pair(ap(lambda x: x * b + 4, b), b)[0]))
    return x


def h2(a, b):
    x = (pair((9 * a), (7 + 15))[0] ^ ((19 + 13) ^ 12))
    return h1(a, 2)


def main():
    y = 0
    f = lambda x: x * 1 + y
    l = build(4, y)
    return ((ap(lambda x: x * pair(y, y)[1] + 8, pair(15, y)[1]) - total(build(1, ap(lambda x: x * 8 + 7, 6)))), pair(total(build(3, y)), pair(11, (1 + y))[1])[0], f((ap(lambda x: x * pair(y, y)[1] + 8, pair(15, y)[1]) - total(build(1, ap(lambda x: x * 8 + 7, 6))))) + f(y), total(l) + total(l))
