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
    x = 2
    return total(build(4, total(build(3, (x if 8 < 10 else x)))))


def h1(a, b):
    x = h0(ap(lambda x: x * 1 + 6, (a ^ 12)), pair((b if 12 < b else a), ap(lambda x: x * a + 1, b))[0])
    return (total(build(1, ap(lambda x: x * a + 5, 15))) * pair(total(build(0, a)), (x + a))[1])


def h2(a, b):
    x = (8 * b)
    return a


def main():
    y = h2(total(build(6, total(build(2, 17)))), h1(8, h0(13, 6)))
    f = lambda x: x * pair((19 & 5), y)[0] + y
    l = build(1, y)
    return (total(build(1, (total(build(1, 4)) ^ (16 if 3 < 10 else 10)))), ap(lambda x: x * ((12 + y) + pair(5, 0)[0]) + 4, 3), f(total(build(1, (total(build(1, 4)) ^ (16 if 3 < 10 else 10))))) + f(y), total(l) + total(l))
