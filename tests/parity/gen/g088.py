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
    x = pair(pair(b, ap(lambda x: x * 10 + 0, b))[0], 16)[1]
    return total(build(0, b))


def h1(a, b):
    x = ((pair(2, a)[0] if ap(lambda x: x * b + 1, 17) < (b ^ b) else pair(a, a)[1]) if pair((a * 0), 4)[1] < pair(pair(a, 4)[0], b)[1] else (total(build(6, 10)) ^ 13))
    return 19


def h2(a, b):
    x = pair(total(build(3, (a if b < a else 2))), pair(12, pair(1, 1)[1])[1])[0]
    return (3 + (pair(x, 6)[0] - (0 if a < a else a)))


def main():
    y = ap(lambda x: x * ap(lambda x: x * (17 ^ 2) + 4, ap(lambda x: x * 7 + 2, 8)) + 7, pair(9, ap(lambda x: x * 0 + 3, 17))[0])
    f = lambda x: x * total(build(0, total(build(6, y)))) + y
    l = build(0, y)
    return (h0(pair(ap(lambda x: x * y + 4, 14), (y * 0))[0], total(build(1, (11 + 1)))), pair(4, h1(y, total(build(3, 19))))[1], f(h0(pair(ap(lambda x: x * y + 4, 14), (y * 0))[0], total(build(1, (11 + 1))))) + f(y), total(l) + total(l))
