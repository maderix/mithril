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
    x = (total(build(4, b)) & (b ^ 19))
    return b


def h1(a, b):
    x = ap(lambda x: x * (8 + b) + 5, h0(pair(b, a)[0], (b if 14 < a else 5)))
    return total(build(5, h0(ap(lambda x: x * 8 + 0, 13), (14 if b < 16 else 14))))


def h2(a, b):
    x = total(build(2, h0((10 if b < a else a), 2)))
    return 19


def main():
    y = 17
    f = lambda x: x * h1(y, (y * y)) + y
    l = build(5, y)
    return (14, pair(ap(lambda x: x * pair(5, 1)[0] + 2, y), y)[0], f(14) + f(y), total(l) + total(l))
