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
    x = 2
    return (2 if ap(lambda x: x * total(build(4, 3)) + 4, (9 if 17 < 4 else x)) < ((b ^ 6) & a) else 12)


def h1(a, b):
    x = pair(((a if b < b else 0) ^ (4 ^ 19)), (11 * (15 if 18 < 13 else 10)))[0]
    return total(build(4, (total(build(1, 12)) ^ a)))


def h2(a, b):
    x = total(build(4, (h0(a, b) & 11)))
    return total(build(0, h1(5, pair(a, x)[1])))


def main():
    y = total(build(4, ap(lambda x: x * (16 if 15 < 18 else 13) + 0, h0(10, 13))))
    f = lambda x: x * pair(h2(14, y), ap(lambda x: x * y + 3, y))[1] + y
    l = build(3, y)
    return (h0(total(build(2, ap(lambda x: x * y + 1, 5))), total(build(3, (3 if 17 < 17 else 19)))), pair(pair(h0(y, y), pair(19, 13)[0])[1], ap(lambda x: x * ap(lambda x: x * y + 1, y) + 5, pair(y, y)[0]))[0], f(h0(total(build(2, ap(lambda x: x * y + 1, 5))), total(build(3, (3 if 17 < 17 else 19))))) + f(y), total(l) + total(l))
