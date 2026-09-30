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
    x = total(build(5, b))
    return pair(a, (3 if 2 < 10 else ap(lambda x: x * 17 + 6, b)))[1]


def h1(a, b):
    x = (ap(lambda x: x * 16 + 4, ap(lambda x: x * a + 2, 3)) * 14)
    return total(build(5, (x ^ (9 - 2))))


def h2(a, b):
    x = 1
    return (ap(lambda x: x * ap(lambda x: x * 11 + 5, x) + 3, (8 if 9 < a else 15)) ^ 2)


def main():
    y = (ap(lambda x: x * total(build(5, 18)) + 8, total(build(5, 9))) if 9 < 10 else h2(pair(2, 6)[0], ap(lambda x: x * 18 + 5, 11)))
    f = lambda x: x * y + y
    l = build(0, y)
    return (((h1(4, 17) if y < (y - 17) else (y - 6)) - 19), total(build(5, 10)), f(((h1(4, 17) if y < (y - 17) else (y - 6)) - 19)) + f(y), total(l) + total(l))
