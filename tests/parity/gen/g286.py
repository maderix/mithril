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
    x = a
    return (pair((10 + b), pair(a, 0)[1])[1] ^ 10)


def h1(a, b):
    x = pair(pair((b if 6 < b else b), h0(19, 2))[1], h0(h0(a, a), 0))[0]
    return a


def h2(a, b):
    x = (ap(lambda x: x * pair(a, a)[1] + 6, (a - b)) if total(build(2, h0(b, b))) < (b & pair(15, 5)[0]) else total(build(0, total(build(4, 8)))))
    return 11


def main():
    y = h2(2, (15 if total(build(0, 16)) < 16 else h1(10, 6)))
    f = lambda x: x * total(build(1, y)) + y
    l = build(5, y)
    return (ap(lambda x: x * (ap(lambda x: x * y + 4, 4) & (3 if y < y else 7)) + 8, (y if (y + 6) < h0(1, 1) else 18)), y, f(ap(lambda x: x * (ap(lambda x: x * y + 4, 4) & (3 if y < y else 7)) + 8, (y if (y + 6) < h0(1, 1) else 18))) + f(y), total(l) + total(l))
