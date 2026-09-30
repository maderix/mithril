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
    x = (ap(lambda x: x * (11 if 13 < 18 else 13) + 1, (b if 15 < a else a)) if 12 < b else total(build(5, total(build(6, 1)))))
    return pair(15, pair((11 if 6 < x else 8), (17 if a < b else 14))[1])[0]


def h1(a, b):
    x = pair((ap(lambda x: x * 15 + 5, 12) if ap(lambda x: x * a + 6, 19) < (5 if 11 < b else b) else h0(b, b)), ((a ^ b) if pair(5, 9)[1] < total(build(0, 8)) else a))[1]
    return h0(h0((9 + 8), (b - a)), h0(h0(2, x), pair(10, 11)[0]))


def h2(a, b):
    x = ap(lambda x: x * total(build(3, (15 ^ 17))) + 2, ap(lambda x: x * b + 3, (b if a < 8 else 12)))
    return total(build(3, (7 if pair(16, 12)[1] < ap(lambda x: x * 17 + 2, b) else total(build(3, 2)))))


def main():
    y = (h1((17 if 18 < 5 else 5), 2) & (total(build(1, 3)) - (16 if 19 < 19 else 8)))
    f = lambda x: x * pair(12, ap(lambda x: x * y + 0, y))[1] + y
    l = build(4, y)
    return (y, 2, f(y) + f(y), total(l) + total(l))
