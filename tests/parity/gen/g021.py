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
    x = ((b - pair(a, 19)[0]) - 13)
    return (ap(lambda x: x * (5 + x) + 4, 18) if ap(lambda x: x * pair(a, 14)[1] + 3, (9 if x < 19 else 13)) < ((16 + 10) if x < 9 else 11) else (b + (x if b < 0 else x)))


def h1(a, b):
    x = pair(ap(lambda x: x * b + 4, 14), pair(pair(3, b)[0], h0(a, 9))[1])[1]
    return h0(h0(h0(b, 0), a), h0(a, h0(0, b)))


def h2(a, b):
    x = 19
    return (((13 if x < 1 else x) ^ h0(0, 12)) if total(build(4, (19 ^ 15))) < (total(build(0, b)) - (11 if b < 15 else 12)) else ap(lambda x: x * a + 7, total(build(3, 8))))


def main():
    y = h2(16, total(build(1, total(build(5, 15)))))
    f = lambda x: x * 2 + y
    l = build(3, y)
    return (h1(total(build(1, pair(15, 19)[1])), total(build(5, (y if y < 13 else y)))), 14, f(h1(total(build(1, pair(15, 19)[1])), total(build(5, (y if y < 13 else y))))) + f(y), total(l) + total(l))
