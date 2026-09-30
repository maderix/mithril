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
    x = 14
    return pair((total(build(0, 10)) ^ b), ap(lambda x: x * 15 + 7, ap(lambda x: x * a + 0, 4)))[1]


def h1(a, b):
    x = b
    return ap(lambda x: x * (ap(lambda x: x * 5 + 0, 6) if (b & 14) < h0(x, 6) else a) + 3, (b if h0(17, x) < 18 else 4))


def h2(a, b):
    x = 16
    return ((ap(lambda x: x * 19 + 4, 12) - (b if x < b else 9)) if (h0(b, x) & pair(x, 12)[0]) < ap(lambda x: x * pair(13, 6)[1] + 5, ap(lambda x: x * 8 + 5, 14)) else x)


def main():
    y = h0(h0(pair(14, 19)[0], (10 if 14 < 19 else 4)), 2)
    f = lambda x: x * total(build(6, ap(lambda x: x * 14 + 1, 1))) + y
    l = build(1, y)
    return ((pair(h1(y, 7), (y if y < 0 else y))[0] * 17), total(build(3, h0((y - y), total(build(4, 15))))), f((pair(h1(y, 7), (y if y < 0 else y))[0] * 17)) + f(y), total(l) + total(l))
