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
    x = 6
    return (18 * 12)


def h1(a, b):
    x = (pair(b, (19 if b < 15 else b))[1] + pair(pair(a, b)[1], total(build(1, a)))[1])
    return pair(5, ap(lambda x: x * total(build(6, 18)) + 8, h0(9, b)))[1]


def h2(a, b):
    x = pair(h1(b, (3 if a < a else 12)), ap(lambda x: x * total(build(1, b)) + 8, 16))[1]
    return b


def main():
    y = total(build(5, ((17 if 17 < 6 else 19) if 7 < ap(lambda x: x * 17 + 1, 18) else ap(lambda x: x * 7 + 5, 14))))
    f = lambda x: x * total(build(4, ap(lambda x: x * y + 2, 16))) + y
    l = build(1, y)
    return (pair(h1((y ^ y), h0(y, y)), total(build(1, (y & 3))))[1], total(build(5, pair((15 + y), pair(8, 11)[1])[0])), f(pair(h1((y ^ y), h0(y, y)), total(build(1, (y & 3))))[1]) + f(y), total(l) + total(l))
