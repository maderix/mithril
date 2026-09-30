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
    x = total(build(1, a))
    return total(build(4, (pair(11, 7)[1] if pair(19, 14)[1] < x else (4 if 1 < a else a))))


def h1(a, b):
    x = total(build(1, b))
    return ap(lambda x: x * ap(lambda x: x * (x if x < 10 else a) + 7, (x & 3)) + 0, a)


def h2(a, b):
    x = total(build(6, ap(lambda x: x * ap(lambda x: x * 5 + 2, 17) + 6, 18)))
    return 9


def main():
    y = 0
    f = lambda x: x * (y if h0(y, 9) < h1(y, y) else ap(lambda x: x * y + 1, 19)) + y
    l = build(2, y)
    return (pair(ap(lambda x: x * ap(lambda x: x * y + 2, 3) + 4, h1(y, 2)), h1(total(build(4, y)), (y if y < 1 else 19)))[1], total(build(3, ((4 if y < 0 else 9) if 1 < pair(8, 14)[0] else y))), f(pair(ap(lambda x: x * ap(lambda x: x * y + 2, 3) + 4, h1(y, 2)), h1(total(build(4, y)), (y if y < 1 else 19)))[1]) + f(y), total(l) + total(l))
