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
    x = 7
    return a


def h1(a, b):
    x = total(build(1, pair(h0(a, 7), pair(3, a)[0])[0]))
    return total(build(6, 18))


def h2(a, b):
    x = 6
    return pair(total(build(1, 11)), pair(pair(x, b)[0], pair(7, a)[0])[1])[0]


def main():
    y = ap(lambda x: x * pair((5 + 17), (4 + 10))[1] + 4, 10)
    f = lambda x: x * total(build(1, h0(8, 5))) + y
    l = build(5, y)
    return (y, pair((pair(17, y)[1] if pair(y, 6)[0] < (15 ^ y) else (y + 15)), y)[1], f(y) + f(y), total(l) + total(l))
