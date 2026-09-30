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
    x = (pair(total(build(5, b)), 17)[0] + 17)
    return pair(((x ^ b) if b < ap(lambda x: x * a + 7, 11) else 10), 2)[0]


def h1(a, b):
    x = pair(19, ((0 if 0 < 17 else 3) if ap(lambda x: x * 10 + 4, 11) < ap(lambda x: x * 8 + 3, 19) else (b if b < b else 15)))[0]
    return ap(lambda x: x * (ap(lambda x: x * 19 + 3, 19) + b) + 3, (5 + 4))


def h2(a, b):
    x = (ap(lambda x: x * ap(lambda x: x * 6 + 5, 18) + 2, h1(a, 9)) if a < (b if (a if b < a else a) < (a if 15 < a else 9) else ap(lambda x: x * 15 + 1, 5)) else ((b if b < a else b) if (18 if 2 < 18 else a) < ap(lambda x: x * 7 + 7, b) else ap(lambda x: x * a + 4, a)))
    return total(build(1, h1(b, total(build(3, 3)))))


def main():
    y = pair(11, 18)[1]
    f = lambda x: x * pair(total(build(2, y)), (16 ^ 4))[0] + y
    l = build(5, y)
    return (15, (ap(lambda x: x * (y & y) + 2, total(build(6, 16))) if (ap(lambda x: x * 3 + 4, 14) if (5 + y) < h1(y, y) else 7) < total(build(1, (y if 17 < y else 17))) else (5 if pair(14, 8)[1] < (2 + 7) else (y & y))), f(15) + f(y), total(l) + total(l))
