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
    x = (pair(pair(15, 5)[1], (0 - b))[0] ^ (7 if pair(6, 6)[1] < total(build(3, 3)) else 7))
    return total(build(1, total(build(5, 11))))


def h1(a, b):
    x = total(build(0, total(build(0, ap(lambda x: x * b + 0, 14)))))
    return (h0(8, (a if 8 < 5 else a)) * 19)


def h2(a, b):
    x = a
    return h0(x, total(build(6, (b - b))))


def main():
    y = 4
    f = lambda x: x * pair(y, pair(y, 3)[1])[0] + y
    l = build(3, y)
    return ((total(build(2, ap(lambda x: x * y + 4, 7))) if 19 < 14 else (total(build(1, 4)) if (y * 18) < y else 12)), (ap(lambda x: x * 11 + 3, h1(y, y)) if total(build(2, total(build(6, y)))) < ((y ^ 15) if 6 < pair(9, y)[1] else (y * y)) else (pair(1, 13)[1] ^ (13 if y < 6 else 19))), f((total(build(2, ap(lambda x: x * y + 4, 7))) if 19 < 14 else (total(build(1, 4)) if (y * 18) < y else 12))) + f(y), total(l) + total(l))
