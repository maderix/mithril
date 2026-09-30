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
    x = ap(lambda x: x * (pair(a, a)[0] if pair(a, 19)[0] < total(build(1, 18)) else 6) + 0, a)
    return 17


def h1(a, b):
    x = pair(h0(total(build(0, b)), 11), total(build(0, (b if 1 < b else b))))[0]
    return ap(lambda x: x * h0(b, h0(8, 11)) + 6, total(build(2, total(build(0, 17)))))


def h2(a, b):
    x = total(build(6, (b if (b - a) < a else 2)))
    return total(build(4, (h0(a, x) if pair(b, a)[0] < ap(lambda x: x * 15 + 7, 2) else a)))


def main():
    y = h2((total(build(6, 9)) ^ 4), h0(pair(10, 13)[1], 6))
    f = lambda x: x * h0(ap(lambda x: x * y + 3, 19), ap(lambda x: x * 3 + 6, 15)) + y
    l = build(0, y)
    return ((ap(lambda x: x * ap(lambda x: x * y + 2, y) + 0, total(build(4, 17))) if 4 < ap(lambda x: x * 15 + 4, ap(lambda x: x * 8 + 1, 15)) else y), (y if h2((y + y), (9 if 10 < 6 else 9)) < h0((18 if 7 < 7 else y), y) else (ap(lambda x: x * y + 1, 16) - total(build(2, 5)))), f((ap(lambda x: x * ap(lambda x: x * y + 2, y) + 0, total(build(4, 17))) if 4 < ap(lambda x: x * 15 + 4, ap(lambda x: x * 8 + 1, 15)) else y)) + f(y), total(l) + total(l))
