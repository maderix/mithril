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
    return total(build(5, total(build(5, x))))


def h1(a, b):
    x = pair(ap(lambda x: x * total(build(6, 13)) + 1, a), (total(build(2, 0)) if pair(a, 13)[1] < (11 if 5 < a else 3) else (17 - b)))[0]
    return h0(ap(lambda x: x * (6 * b) + 1, total(build(3, x))), total(build(5, pair(0, 4)[0])))


def h2(a, b):
    x = 2
    return ap(lambda x: x * h0(ap(lambda x: x * a + 1, 9), (b if a < b else a)) + 5, x)


def main():
    y = pair(1, total(build(3, pair(14, 8)[1])))[0]
    f = lambda x: x * ap(lambda x: x * h0(0, y) + 0, (8 if 3 < 10 else y)) + y
    l = build(4, y)
    return (10, y, f(10) + f(y), total(l) + total(l))
