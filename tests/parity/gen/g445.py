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
    x = 7
    return ap(lambda x: x * 4 + 7, total(build(2, 14)))


def h1(a, b):
    x = pair(a, h0(b, total(build(5, 11))))[1]
    return 5


def h2(a, b):
    x = total(build(1, pair(3, h1(10, b))[0]))
    return (h1(h1(16, x), ap(lambda x: x * x + 0, b)) ^ pair(a, (a if b < a else x))[1])


def main():
    y = total(build(3, h1((4 ^ 8), pair(9, 19)[0])))
    f = lambda x: x * ap(lambda x: x * y + 6, (y if 11 < y else y)) + y
    l = build(4, y)
    return (y, pair(y, pair(pair(y, 3)[0], (y if y < 11 else 10))[0])[0], f(y) + f(y), total(l) + total(l))
