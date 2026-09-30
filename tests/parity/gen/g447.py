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
    x = (1 + pair((b ^ 1), 15)[0])
    return x


def h1(a, b):
    x = total(build(3, (pair(4, b)[0] if 6 < ap(lambda x: x * 11 + 5, a) else h0(b, b))))
    return h0(h0(h0(a, a), (b if 1 < 6 else 8)), (h0(a, a) if 8 < 2 else 18))


def h2(a, b):
    x = ((4 ^ b) * h0(ap(lambda x: x * 1 + 4, a), pair(10, a)[0]))
    return total(build(4, h0(b, h0(b, x))))


def main():
    y = pair(3, (pair(14, 19)[0] if pair(9, 9)[0] < h0(3, 5) else ap(lambda x: x * 10 + 3, 12)))[1]
    f = lambda x: x * (total(build(0, y)) if y < total(build(6, 9)) else 13) + y
    l = build(0, y)
    return (ap(lambda x: x * (pair(y, 15)[1] if 10 < h0(y, y) else (y * 15)) + 7, (total(build(4, y)) & total(build(2, 19)))), h0(ap(lambda x: x * ap(lambda x: x * 12 + 0, y) + 8, (12 + 17)), ((12 + y) ^ h0(y, y))), f(ap(lambda x: x * (pair(y, 15)[1] if 10 < h0(y, y) else (y * 15)) + 7, (total(build(4, y)) & total(build(2, 19))))) + f(y), total(l) + total(l))
