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
    x = pair(total(build(0, pair(b, 9)[0])), 18)[0]
    return total(build(4, (pair(x, 18)[1] if 10 < ap(lambda x: x * b + 4, 16) else ap(lambda x: x * a + 8, 14))))


def h1(a, b):
    x = total(build(3, ((3 * a) if (b if b < b else 6) < pair(8, b)[0] else b)))
    return (ap(lambda x: x * h0(b, 2) + 4, total(build(0, 19))) if h0(ap(lambda x: x * b + 7, b), total(build(2, 8))) < 10 else (h0(b, 7) + 7))


def h2(a, b):
    x = ap(lambda x: x * (total(build(0, 2)) & b) + 5, h1(total(build(2, 13)), 17))
    return 18


def main():
    y = 13
    f = lambda x: x * y + y
    l = build(2, y)
    return (y, ap(lambda x: x * h0((y & y), ap(lambda x: x * 11 + 2, y)) + 1, ap(lambda x: x * y + 4, h1(y, 10))), f(y) + f(y), total(l) + total(l))
