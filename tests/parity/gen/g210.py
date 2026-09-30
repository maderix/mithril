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
    x = b
    return total(build(3, total(build(1, total(build(2, a))))))


def h1(a, b):
    x = 18
    return h0(total(build(0, (b ^ 3))), total(build(2, (a if x < 0 else b))))


def h2(a, b):
    x = (b if ap(lambda x: x * (12 if 6 < b else b) + 0, (17 ^ b)) < pair((5 + b), pair(a, a)[1])[1] else b)
    return (pair((8 if x < b else 9), b)[1] if ap(lambda x: x * 10 + 2, (x if 1 < b else a)) < total(build(4, (4 * a))) else h1((a if a < 1 else b), pair(b, 17)[0]))


def main():
    y = (7 if 14 < h1(ap(lambda x: x * 14 + 0, 11), 3) else h0(ap(lambda x: x * 4 + 5, 18), h2(14, 2)))
    f = lambda x: x * pair((y if y < 4 else y), ap(lambda x: x * 15 + 1, y))[1] + y
    l = build(5, y)
    return (y, (5 ^ h1(total(build(6, y)), pair(15, y)[1])), f(y) + f(y), total(l) + total(l))
