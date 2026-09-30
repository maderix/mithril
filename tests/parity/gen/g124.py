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
    return b


def h1(a, b):
    x = ((pair(15, 9)[1] if (b - 7) < h0(19, 3) else h0(15, a)) if h0(4, total(build(4, 6))) < 7 else ap(lambda x: x * h0(a, 3) + 7, total(build(5, 7))))
    return pair(h0((9 if b < b else b), pair(b, x)[0]), 12)[0]


def h2(a, b):
    x = ap(lambda x: x * h1(b, h1(17, 13)) + 6, b)
    return b


def main():
    y = pair(total(build(0, (18 & 4))), pair(total(build(4, 17)), 12)[0])[1]
    f = lambda x: x * pair(h2(11, y), h0(y, 12))[1] + y
    l = build(0, y)
    return ((y * y), y, f((y * y)) + f(y), total(l) + total(l))
