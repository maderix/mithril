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
    x = (ap(lambda x: x * (a & a) + 6, ap(lambda x: x * 1 + 0, 0)) * total(build(2, total(build(6, 10)))))
    return ap(lambda x: x * total(build(0, (9 if 15 < a else b))) + 6, a)


def h1(a, b):
    x = pair(h0(4, (11 if 12 < 19 else a)), b)[0]
    return total(build(2, h0(x, h0(8, b))))


def h2(a, b):
    x = ((total(build(6, b)) + pair(17, a)[1]) - a)
    return h1(total(build(6, 17)), ap(lambda x: x * (a & b) + 2, (x + 6)))


def main():
    y = ap(lambda x: x * 17 + 8, ap(lambda x: x * (12 if 7 < 10 else 0) + 4, 19))
    f = lambda x: x * pair(total(build(1, y)), ap(lambda x: x * y + 4, 11))[0] + y
    l = build(3, y)
    return ((ap(lambda x: x * (y - y) + 0, total(build(6, 7))) & pair(y, (9 if y < 19 else 6))[0]), ((1 - h1(0, y)) if (15 & total(build(1, 12))) < (y ^ (7 * 8)) else 2), f((ap(lambda x: x * (y - y) + 0, total(build(6, 7))) & pair(y, (9 if y < 19 else 6))[0])) + f(y), total(l) + total(l))
