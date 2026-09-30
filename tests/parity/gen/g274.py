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
    x = pair(ap(lambda x: x * b + 5, 17), 6)[0]
    return ap(lambda x: x * 6 + 7, total(build(6, total(build(0, x)))))


def h1(a, b):
    x = ap(lambda x: x * h0(b, (11 if b < 19 else 11)) + 8, pair((1 if a < b else a), (b - b))[1])
    return total(build(5, ((a ^ 9) & h0(10, b))))


def h2(a, b):
    x = h0((4 if total(build(4, b)) < (16 & b) else h1(18, b)), (19 * total(build(3, 13))))
    return (total(build(6, ap(lambda x: x * x + 7, a))) if a < total(build(2, total(build(3, x)))) else total(build(6, pair(b, b)[0])))


def main():
    y = (pair(8, h2(4, 14))[1] if (h2(18, 9) ^ total(build(2, 17))) < h1((19 if 0 < 9 else 9), h0(11, 5)) else total(build(5, ap(lambda x: x * 1 + 3, 17))))
    f = lambda x: x * h0((y if 7 < y else 13), h0(y, 19)) + y
    l = build(1, y)
    return ((ap(lambda x: x * total(build(3, y)) + 7, 4) if h1((13 if y < 10 else 3), y) < total(build(1, pair(9, y)[0])) else ((y * 15) if ap(lambda x: x * y + 6, y) < h2(y, 14) else pair(y, 13)[1])), 13, f((ap(lambda x: x * total(build(3, y)) + 7, 4) if h1((13 if y < 10 else 3), y) < total(build(1, pair(9, y)[0])) else ((y * 15) if ap(lambda x: x * y + 6, y) < h2(y, 14) else pair(y, 13)[1]))) + f(y), total(l) + total(l))
