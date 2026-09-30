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
    x = pair(ap(lambda x: x * 7 + 3, a), (ap(lambda x: x * 8 + 3, 0) * (12 if 18 < b else b)))[0]
    return 2


def h1(a, b):
    x = pair(15, ap(lambda x: x * (11 - 5) + 3, ap(lambda x: x * 7 + 7, 10)))[0]
    return total(build(3, ap(lambda x: x * (7 if 3 < 4 else a) + 8, (4 + x))))


def h2(a, b):
    x = total(build(5, total(build(6, pair(b, a)[0]))))
    return a


def main():
    y = 15
    f = lambda x: x * total(build(0, h1(y, 12))) + y
    l = build(1, y)
    return (total(build(5, h0((y & y), 15))), pair(total(build(6, 6)), 8)[1], f(total(build(5, h0((y & y), 15)))) + f(y), total(l) + total(l))
