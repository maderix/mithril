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
    x = ((ap(lambda x: x * 11 + 3, 3) ^ total(build(0, 6))) if pair((15 if 18 < 11 else b), (b + a))[0] < 11 else total(build(1, pair(13, 10)[0])))
    return total(build(4, total(build(6, 6))))


def h1(a, b):
    x = h0(4, a)
    return h0(ap(lambda x: x * h0(x, a) + 1, b), (b - (b ^ a)))


def h2(a, b):
    x = total(build(5, pair((5 if a < 5 else b), (0 & a))[1]))
    return 11


def main():
    y = total(build(0, (h1(14, 17) if h1(5, 1) < (19 + 11) else (2 if 13 < 12 else 15))))
    f = lambda x: x * 18 + y
    l = build(5, y)
    return (total(build(6, ((y if 9 < y else 12) ^ ap(lambda x: x * y + 8, 10)))), y, f(total(build(6, ((y if 9 < y else 12) ^ ap(lambda x: x * y + 8, 10))))) + f(y), total(l) + total(l))
