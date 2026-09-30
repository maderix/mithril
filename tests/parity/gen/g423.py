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
    x = total(build(2, ((14 if a < a else b) - 14)))
    return total(build(3, pair(ap(lambda x: x * b + 3, b), 8)[1]))


def h1(a, b):
    x = (ap(lambda x: x * (15 if a < b else 13) + 1, ap(lambda x: x * a + 3, 9)) - b)
    return pair(pair(total(build(6, 9)), ap(lambda x: x * 6 + 2, 13))[1], total(build(6, 13)))[1]


def h2(a, b):
    x = (h1(total(build(1, b)), h0(0, b)) if ap(lambda x: x * a + 6, pair(a, a)[1]) < b else pair(h1(18, a), pair(4, 1)[1])[1])
    return ap(lambda x: x * a + 6, h1(8, h0(a, a)))


def main():
    y = pair(12, ap(lambda x: x * total(build(4, 15)) + 3, h0(12, 1)))[0]
    f = lambda x: x * ap(lambda x: x * (1 if 1 < y else y) + 0, (8 - 19)) + y
    l = build(2, y)
    return (y, 17, f(y) + f(y), total(l) + total(l))
