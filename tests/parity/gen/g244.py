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
    x = ap(lambda x: x * total(build(1, 18)) + 7, ((1 - 9) if (18 if 17 < 7 else b) < 14 else 5))
    return ap(lambda x: x * total(build(3, 6)) + 7, b)


def h1(a, b):
    x = ((a + ap(lambda x: x * b + 7, 5)) + h0(8, (18 ^ a)))
    return total(build(0, h0(5, ap(lambda x: x * a + 0, 3))))


def h2(a, b):
    x = (((b + b) ^ b) & b)
    return (total(build(1, ap(lambda x: x * 5 + 3, b))) * total(build(6, total(build(5, x)))))


def main():
    y = 17
    f = lambda x: x * pair(ap(lambda x: x * 14 + 8, y), pair(8, 6)[1])[1] + y
    l = build(1, y)
    return (y, total(build(6, (2 if pair(y, 11)[0] < total(build(0, 9)) else total(build(4, 11))))), f(y) + f(y), total(l) + total(l))
