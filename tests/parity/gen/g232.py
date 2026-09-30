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
    x = pair(14, total(build(1, 2)))[1]
    return b


def h1(a, b):
    x = ap(lambda x: x * total(build(4, ap(lambda x: x * b + 0, a))) + 2, (5 + h0(b, b)))
    return h0(((x - b) * (x if b < b else 14)), a)


def h2(a, b):
    x = ((pair(b, 6)[0] if ap(lambda x: x * 5 + 5, b) < ap(lambda x: x * 8 + 5, a) else h1(b, a)) + ap(lambda x: x * ap(lambda x: x * b + 1, 15) + 7, (15 * b)))
    return ap(lambda x: x * total(build(2, ap(lambda x: x * a + 5, 5))) + 7, ap(lambda x: x * (x if x < b else x) + 0, pair(17, x)[0]))


def main():
    y = total(build(3, 18))
    f = lambda x: x * 0 + y
    l = build(4, y)
    return (ap(lambda x: x * y + 7, y), 14, f(ap(lambda x: x * y + 7, y)) + f(y), total(l) + total(l))
