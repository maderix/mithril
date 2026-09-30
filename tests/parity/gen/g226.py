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
    x = total(build(2, total(build(5, b))))
    return total(build(1, total(build(6, (x & 4)))))


def h1(a, b):
    x = h0(pair(total(build(5, b)), a)[0], ap(lambda x: x * pair(a, 13)[1] + 0, ap(lambda x: x * 1 + 1, 5)))
    return total(build(5, (b if h0(x, a) < ap(lambda x: x * 16 + 5, 3) else b)))


def h2(a, b):
    x = total(build(0, pair(4, total(build(5, a)))[0]))
    return total(build(1, pair(total(build(4, 15)), total(build(5, x)))[1]))


def main():
    y = (ap(lambda x: x * 2 + 4, (19 if 19 < 1 else 19)) + ap(lambda x: x * pair(0, 14)[0] + 8, 16))
    f = lambda x: x * h1(0, (y if y < y else y)) + y
    l = build(3, y)
    return (total(build(5, (pair(y, 2)[0] + (y if y < 0 else y)))), 12, f(total(build(5, (pair(y, 2)[0] + (y if y < 0 else y))))) + f(y), total(l) + total(l))
