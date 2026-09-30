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
    x = (total(build(6, total(build(1, b)))) if 13 < ((b ^ 2) if (a if a < 8 else 9) < (2 + b) else 13) else total(build(6, total(build(6, a)))))
    return 12


def h1(a, b):
    x = pair(pair(a, total(build(2, b)))[0], pair(12, total(build(1, a)))[1])[0]
    return total(build(5, 5))


def h2(a, b):
    x = a
    return h0(b, (x & 4))


def main():
    y = 14
    f = lambda x: x * total(build(5, y)) + y
    l = build(4, y)
    return ((16 if 10 < ap(lambda x: x * 9 + 8, 12) else h2(total(build(4, 3)), total(build(0, 7)))), pair(((y if 7 < 1 else 16) * ap(lambda x: x * y + 5, y)), total(build(3, pair(y, 12)[0])))[0], f((16 if 10 < ap(lambda x: x * 9 + 8, 12) else h2(total(build(4, 3)), total(build(0, 7))))) + f(y), total(l) + total(l))
