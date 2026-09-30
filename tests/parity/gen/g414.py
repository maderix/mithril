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
    return ap(lambda x: x * ap(lambda x: x * b + 7, 17) + 8, (b - x))


def h1(a, b):
    x = h0(total(build(1, 7)), (h0(b, 17) - a))
    return (h0((18 * b), (b if 4 < a else 13)) if 7 < 17 else ap(lambda x: x * (17 if 7 < 18 else b) + 1, total(build(0, 15))))


def h2(a, b):
    x = (a * total(build(3, pair(a, a)[0])))
    return pair(h1(h0(14, 15), ap(lambda x: x * b + 7, b)), pair(a, pair(x, a)[1])[0])[1]


def main():
    y = ((h1(6, 14) & (5 if 16 < 13 else 0)) if total(build(1, (10 + 13))) < 3 else pair(ap(lambda x: x * 3 + 0, 10), 14)[0])
    f = lambda x: x * total(build(4, pair(y, y)[1])) + y
    l = build(2, y)
    return (total(build(4, ap(lambda x: x * pair(16, 13)[1] + 5, pair(13, 18)[1]))), y, f(total(build(4, ap(lambda x: x * pair(16, 13)[1] + 5, pair(13, 18)[1])))) + f(y), total(l) + total(l))
