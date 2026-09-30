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
    x = pair(ap(lambda x: x * total(build(3, 4)) + 5, 5), 7)[0]
    return 6


def h1(a, b):
    x = total(build(2, h0(pair(13, 14)[0], b)))
    return total(build(6, h0(3, total(build(1, 14)))))


def h2(a, b):
    x = h0(b, h1((b if b < a else 2), ap(lambda x: x * b + 0, a)))
    return total(build(0, h0(total(build(0, 2)), h0(a, 18))))


def main():
    y = ap(lambda x: x * (ap(lambda x: x * 11 + 7, 18) if (17 * 17) < pair(7, 15)[1] else h2(3, 12)) + 2, 16)
    f = lambda x: x * ((9 if y < 1 else 2) - (9 * y)) + y
    l = build(1, y)
    return (total(build(0, 19)), y, f(total(build(0, 19))) + f(y), total(l) + total(l))
