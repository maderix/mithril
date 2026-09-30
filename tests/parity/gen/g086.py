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
    return pair(x, total(build(5, pair(13, x)[0])))[0]


def h1(a, b):
    x = (ap(lambda x: x * h0(2, 3) + 6, (b - 6)) if pair((14 - 12), total(build(5, b)))[0] < (total(build(3, 0)) ^ ap(lambda x: x * b + 7, a)) else h0(pair(19, a)[1], pair(a, b)[1]))
    return h0(pair(total(build(5, a)), h0(4, x))[0], pair(ap(lambda x: x * 15 + 6, x), h0(5, x))[1])


def h2(a, b):
    x = ap(lambda x: x * ap(lambda x: x * 0 + 7, total(build(5, 5))) + 6, h1((15 if 10 < 4 else 15), pair(a, 9)[0]))
    return total(build(2, ap(lambda x: x * x + 8, (4 - x))))


def main():
    y = pair(((5 ^ 13) & 6), pair(pair(17, 5)[0], 2)[1])[0]
    f = lambda x: x * h0((1 - y), (9 if y < y else 5)) + y
    l = build(2, y)
    return (ap(lambda x: x * h1((7 & y), 18) + 6, total(build(6, total(build(3, 17))))), (h0(16, pair(y, y)[1]) if pair(9, (1 - 0))[0] < (y + ap(lambda x: x * 15 + 0, 1)) else h2(pair(y, y)[1], (16 - y))), f(ap(lambda x: x * h1((7 & y), 18) + 6, total(build(6, total(build(3, 17)))))) + f(y), total(l) + total(l))
