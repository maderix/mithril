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
    x = (pair(total(build(2, 17)), (b if 1 < b else b))[0] if 4 < a else ap(lambda x: x * 15 + 4, (0 if b < 2 else b)))
    return 6


def h1(a, b):
    x = h0(h0(18, ap(lambda x: x * 4 + 8, b)), h0(h0(b, b), b))
    return x


def h2(a, b):
    x = pair(total(build(1, h1(3, 19))), ap(lambda x: x * ap(lambda x: x * 8 + 6, b) + 3, 13))[1]
    return total(build(0, total(build(5, (x + x)))))


def main():
    y = h0((ap(lambda x: x * 15 + 6, 5) + (9 - 17)), 7)
    f = lambda x: x * h1(pair(y, 8)[0], 10) + y
    l = build(6, y)
    return ((y if h0(h0(y, 16), total(build(2, y))) < (total(build(4, 11)) + h2(y, 6)) else total(build(5, pair(y, y)[0]))), h2(h0(h1(9, 1), ap(lambda x: x * 9 + 3, y)), 9), f((y if h0(h0(y, 16), total(build(2, y))) < (total(build(4, 11)) + h2(y, 6)) else total(build(5, pair(y, y)[0])))) + f(y), total(l) + total(l))
