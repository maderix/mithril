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
    x = (((b ^ 10) & 17) if 7 < (ap(lambda x: x * b + 8, b) * (a * a)) else pair((11 - a), b)[1])
    return total(build(4, ap(lambda x: x * (x * 3) + 7, 5)))


def h1(a, b):
    x = h0(pair((a & b), (a if 17 < 1 else b))[0], total(build(6, h0(15, a))))
    return pair(h0(17, b), h0((x ^ 8), (a - a)))[1]


def h2(a, b):
    x = ((pair(2, 9)[1] if (12 - 11) < total(build(5, b)) else h1(10, a)) * a)
    return h1(((a * x) if b < b else ap(lambda x: x * 15 + 7, x)), pair(pair(1, b)[0], (b if 19 < 13 else 8))[0])


def main():
    y = 14
    f = lambda x: x * ap(lambda x: x * y + 2, (y - y)) + y
    l = build(4, y)
    return (17, 10, f(17) + f(y), total(l) + total(l))
