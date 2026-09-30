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
    x = a
    return (ap(lambda x: x * ap(lambda x: x * 10 + 8, x) + 0, pair(11, a)[0]) + ap(lambda x: x * 10 + 1, (11 if 17 < a else a)))


def h1(a, b):
    x = h0(total(build(0, ap(lambda x: x * 15 + 4, a))), (h0(2, 2) + (5 - 19)))
    return h0(total(build(3, total(build(4, a)))), x)


def h2(a, b):
    x = pair(h0(h1(a, 5), (b + a)), (a ^ b))[0]
    return pair(total(build(0, h1(b, 4))), 4)[1]


def main():
    y = total(build(1, pair(13, total(build(6, 8)))[0]))
    f = lambda x: x * y + y
    l = build(0, y)
    return (total(build(4, total(build(4, y)))), (y ^ ap(lambda x: x * y + 4, ap(lambda x: x * y + 3, 11))), f(total(build(4, total(build(4, y))))) + f(y), total(l) + total(l))
