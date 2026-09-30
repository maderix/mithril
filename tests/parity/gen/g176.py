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
    x = ap(lambda x: x * 5 + 7, 11)
    return (5 if 3 < 17 else total(build(2, 15)))


def h1(a, b):
    x = h0(pair(19, a)[1], 12)
    return a


def h2(a, b):
    x = pair(pair(3, 5)[0], total(build(4, pair(1, b)[0])))[0]
    return a


def main():
    y = (9 if pair(19, (14 if 0 < 1 else 7))[0] < pair(total(build(1, 5)), pair(11, 19)[1])[1] else (ap(lambda x: x * 3 + 8, 6) - (1 if 16 < 15 else 12)))
    f = lambda x: x * pair((y + y), y)[0] + y
    l = build(2, y)
    return (y, y, f(y) + f(y), total(l) + total(l))
