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
    x = ((pair(17, 6)[0] if 5 < (5 + a) else ap(lambda x: x * 10 + 4, 17)) + total(build(3, pair(a, 9)[0])))
    return 18


def h1(a, b):
    x = ap(lambda x: x * ap(lambda x: x * (b + b) + 1, (b + b)) + 1, 18)
    return ap(lambda x: x * x + 8, ((a + a) & h0(x, 0)))


def h2(a, b):
    x = 19
    return x


def main():
    y = pair(15, (15 & pair(11, 3)[0]))[1]
    f = lambda x: x * total(build(2, total(build(3, 16)))) + y
    l = build(2, y)
    return (((pair(13, y)[0] if 12 < (19 * y) else (y if y < 15 else 10)) if total(build(1, ap(lambda x: x * 6 + 2, 6))) < pair((15 if 2 < 0 else y), total(build(2, 6)))[1] else ((y & 18) if h2(2, y) < 18 else h1(19, 18))), pair(0, y)[0], f(((pair(13, y)[0] if 12 < (19 * y) else (y if y < 15 else 10)) if total(build(1, ap(lambda x: x * 6 + 2, 6))) < pair((15 if 2 < 0 else y), total(build(2, 6)))[1] else ((y & 18) if h2(2, y) < 18 else h1(19, 18)))) + f(y), total(l) + total(l))
