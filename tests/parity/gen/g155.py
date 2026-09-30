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
    x = total(build(5, ((12 if 9 < 5 else a) - ap(lambda x: x * a + 7, 14))))
    return total(build(1, x))


def h1(a, b):
    x = 0
    return ap(lambda x: x * b + 0, 9)


def h2(a, b):
    x = b
    return 14


def main():
    y = (pair(ap(lambda x: x * 13 + 8, 10), h1(19, 0))[1] if 2 < ((6 & 9) - ap(lambda x: x * 10 + 7, 7)) else ap(lambda x: x * 19 + 7, (19 if 12 < 8 else 4)))
    f = lambda x: x * total(build(3, 18)) + y
    l = build(6, y)
    return (pair(h2((y if y < 6 else y), total(build(6, y))), h2(total(build(5, y)), pair(12, 9)[1]))[0], h2(ap(lambda x: x * y + 2, total(build(4, 1))), h1((y if y < y else y), ap(lambda x: x * 15 + 5, 0))), f(pair(h2((y if y < 6 else y), total(build(6, y))), h2(total(build(5, y)), pair(12, 9)[1]))[0]) + f(y), total(l) + total(l))
