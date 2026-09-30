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
    x = 3
    return b


def h1(a, b):
    x = h0(pair((b & 12), h0(11, b))[0], total(build(6, (14 if a < b else 1))))
    return total(build(1, total(build(0, x))))


def h2(a, b):
    x = total(build(3, b))
    return a


def main():
    y = 4
    f = lambda x: x * y + y
    l = build(5, y)
    return (pair(total(build(6, h0(0, y))), 18)[0], (((13 if 9 < y else y) * total(build(2, 6))) ^ y), f(pair(total(build(6, h0(0, y))), 18)[0]) + f(y), total(l) + total(l))
