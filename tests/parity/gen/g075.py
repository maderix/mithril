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
    x = (ap(lambda x: x * 2 + 3, total(build(4, 5))) if a < (18 if (b if b < b else 4) < pair(17, 12)[1] else a) else total(build(5, (18 - a))))
    return pair(pair(x, (b & x))[0], ((9 * x) ^ b))[0]


def h1(a, b):
    x = h0(h0(pair(b, a)[1], 18), pair(b, total(build(0, b)))[0])
    return total(build(0, ((2 ^ 9) * pair(b, 17)[1])))


def h2(a, b):
    x = (h0(b, (a * 9)) if total(build(5, b)) < (h1(17, 9) & b) else total(build(3, b)))
    return (b & pair(total(build(5, x)), 10)[0])


def main():
    y = h1(h2((10 ^ 18), (18 if 6 < 10 else 1)), h2(h1(2, 9), total(build(0, 18))))
    f = lambda x: x * 16 + y
    l = build(1, y)
    return ((ap(lambda x: x * (11 if y < 4 else 2) + 3, h0(4, y)) & h0(total(build(1, 1)), pair(y, 7)[1])), 15, f((ap(lambda x: x * (11 if y < 4 else 2) + 3, h0(4, y)) & h0(total(build(1, 1)), pair(y, 7)[1]))) + f(y), total(l) + total(l))
