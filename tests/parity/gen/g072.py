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
    x = (total(build(5, ap(lambda x: x * 12 + 1, 6))) - total(build(4, 8)))
    return b


def h1(a, b):
    x = h0(b, total(build(0, h0(b, b))))
    return pair(pair(11, pair(b, 19)[0])[0], b)[0]


def h2(a, b):
    x = h1(((a + b) if ap(lambda x: x * b + 3, a) < (8 if b < b else a) else ap(lambda x: x * a + 8, a)), h0((b if a < 0 else a), (a ^ 3)))
    return 8


def main():
    y = pair(h0(pair(0, 12)[1], (5 if 4 < 1 else 15)), ((15 if 14 < 12 else 18) * 11))[1]
    f = lambda x: x * 4 + y
    l = build(2, y)
    return ((h2(pair(1, 1)[0], total(build(6, y))) & h1((18 - 2), pair(18, 0)[0])), 7, f((h2(pair(1, 1)[0], total(build(6, y))) & h1((18 - 2), pair(18, 0)[0]))) + f(y), total(l) + total(l))
