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
    x = pair((4 - ap(lambda x: x * a + 3, 1)), 19)[0]
    return 8


def h1(a, b):
    x = ap(lambda x: x * (6 - (11 * 8)) + 6, (b if pair(6, 1)[1] < (15 + a) else total(build(5, 6))))
    return pair((total(build(4, 16)) ^ ap(lambda x: x * 12 + 7, 8)), (15 if 9 < (b if x < 11 else 15) else h0(x, b)))[0]


def h2(a, b):
    x = total(build(6, (pair(a, b)[0] - pair(8, 19)[0])))
    return total(build(2, total(build(6, ap(lambda x: x * b + 3, b)))))


def main():
    y = 17
    f = lambda x: x * y + y
    l = build(5, y)
    return (y, y, f(y) + f(y), total(l) + total(l))
