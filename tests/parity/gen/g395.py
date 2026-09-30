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
    x = pair(b, pair(2, ap(lambda x: x * b + 2, a))[1])[0]
    return total(build(1, 4))


def h1(a, b):
    x = total(build(3, pair(7, ap(lambda x: x * b + 1, b))[0]))
    return pair((ap(lambda x: x * 11 + 7, 9) if total(build(1, 18)) < x else ap(lambda x: x * b + 0, a)), ap(lambda x: x * (15 + x) + 1, a))[0]


def h2(a, b):
    x = a
    return (pair(pair(b, x)[1], ap(lambda x: x * 3 + 1, x))[0] ^ (total(build(6, 10)) if h0(b, 3) < h1(1, 16) else (16 * 17)))


def main():
    y = (pair((2 & 0), (12 if 4 < 6 else 11))[0] if pair(pair(0, 9)[1], (18 if 8 < 17 else 7))[1] < total(build(0, (6 if 12 < 4 else 13))) else h0(h0(8, 7), (10 + 15)))
    f = lambda x: x * h2((5 if 16 < y else y), (y + 11)) + y
    l = build(4, y)
    return (pair(y, h1((y if 0 < y else 17), 2))[0], h1((h1(y, 11) * (0 if 6 < y else 8)), ((y if y < y else 13) ^ pair(15, 17)[0])), f(pair(y, h1((y if 0 < y else 17), 2))[0]) + f(y), total(l) + total(l))
