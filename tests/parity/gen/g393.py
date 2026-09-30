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
    x = ap(lambda x: x * ((8 if 7 < 5 else b) + 14) + 7, ap(lambda x: x * (b if 2 < 14 else 19) + 3, (3 ^ a)))
    return 7


def h1(a, b):
    x = (ap(lambda x: x * ap(lambda x: x * 4 + 1, b) + 7, b) + 16)
    return b


def h2(a, b):
    x = pair(h1(total(build(3, 4)), h0(1, 6)), pair(2, h1(6, 7))[0])[0]
    return 15


def main():
    y = ((5 - ap(lambda x: x * 8 + 7, 7)) if 9 < (15 ^ ap(lambda x: x * 6 + 4, 11)) else 8)
    f = lambda x: x * 7 + y
    l = build(4, y)
    return (12, pair(pair((y if y < y else 16), (y ^ 7))[0], y)[0], f(12) + f(y), total(l) + total(l))
