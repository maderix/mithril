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
    x = total(build(6, 15))
    return 3


def h1(a, b):
    x = (total(build(6, ap(lambda x: x * b + 2, 3))) - ap(lambda x: x * 3 + 4, pair(a, a)[0]))
    return total(build(3, ap(lambda x: x * pair(10, 0)[1] + 4, ap(lambda x: x * a + 0, b))))


def h2(a, b):
    x = h0(pair((13 if 10 < b else 11), (8 if a < a else 2))[0], pair((b - 9), 0)[1])
    return a


def main():
    y = (0 if total(build(1, 19)) < h0(pair(7, 1)[1], (19 if 18 < 1 else 5)) else pair(total(build(5, 17)), (16 if 0 < 12 else 16))[1])
    f = lambda x: x * y + y
    l = build(4, y)
    return (16, (ap(lambda x: x * total(build(4, 5)) + 0, total(build(5, 17))) * ap(lambda x: x * ap(lambda x: x * 0 + 2, y) + 6, (1 - 19))), f(16) + f(y), total(l) + total(l))
