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
    x = (b if pair(a, b)[1] < (14 + 13) else pair(pair(b, a)[0], 0)[0])
    return b


def h1(a, b):
    x = pair(19, h0(18, pair(b, b)[0]))[0]
    return (2 ^ b)


def h2(a, b):
    x = h1(ap(lambda x: x * ap(lambda x: x * b + 5, 3) + 0, (b if a < 16 else 6)), pair(pair(14, 13)[0], pair(2, 15)[1])[1])
    return h1(h0((7 ^ 4), (x if 9 < a else 9)), x)


def main():
    y = (ap(lambda x: x * ap(lambda x: x * 6 + 0, 19) + 3, 16) if 6 < total(build(1, (19 & 18))) else (total(build(3, 18)) if pair(10, 7)[0] < (15 if 4 < 12 else 6) else (11 - 13)))
    f = lambda x: x * pair(pair(15, 0)[0], (y if y < 15 else y))[0] + y
    l = build(6, y)
    return (h1(total(build(3, 13)), total(build(3, total(build(0, y))))), h2(total(build(4, pair(17, 4)[0])), y), f(h1(total(build(3, 13)), total(build(3, total(build(0, y)))))) + f(y), total(l) + total(l))
