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
    x = 11
    return (a + ((a if b < a else a) if total(build(1, b)) < 2 else ap(lambda x: x * a + 0, b)))


def h1(a, b):
    x = h0(ap(lambda x: x * 0 + 2, 16), h0(total(build(1, 12)), total(build(4, 7))))
    return total(build(4, ((x if 10 < b else 2) * 12)))


def h2(a, b):
    x = pair((h0(a, 1) & ap(lambda x: x * a + 1, a)), ((4 - a) * a))[0]
    return total(build(1, total(build(3, pair(6, 6)[1]))))


def main():
    y = 10
    f = lambda x: x * h1(y, h1(16, 10)) + y
    l = build(6, y)
    return (ap(lambda x: x * pair(ap(lambda x: x * y + 4, y), 1)[0] + 5, (10 + (y if 3 < 4 else 1))), (h1(pair(y, 2)[0], y) if total(build(2, pair(16, 2)[0])) < total(build(3, (15 if y < 2 else y))) else pair((y + y), pair(y, 15)[0])[0]), f(ap(lambda x: x * pair(ap(lambda x: x * y + 4, y), 1)[0] + 5, (10 + (y if 3 < 4 else 1)))) + f(y), total(l) + total(l))
