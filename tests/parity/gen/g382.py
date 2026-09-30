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
    x = a
    return 11


def h1(a, b):
    x = h0(ap(lambda x: x * pair(0, 13)[1] + 1, (11 ^ b)), b)
    return total(build(6, x))


def h2(a, b):
    x = (total(build(0, 0)) - pair(ap(lambda x: x * 8 + 2, a), pair(13, 10)[1])[0])
    return pair(5, x)[1]


def main():
    y = 10
    f = lambda x: x * 17 + y
    l = build(0, y)
    return (16, ap(lambda x: x * (h0(y, 8) if (y + y) < 9 else (y ^ y)) + 7, h1((2 if y < 13 else 11), total(build(2, 13)))), f(16) + f(y), total(l) + total(l))
