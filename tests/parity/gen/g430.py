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
    return 14


def h1(a, b):
    x = total(build(2, (total(build(4, a)) if ap(lambda x: x * a + 1, 0) < (b if a < b else 1) else h0(1, 10))))
    return a


def h2(a, b):
    x = h0(((b if 18 < 15 else 18) ^ (b if 16 < 3 else 17)), ((11 if 10 < a else b) & total(build(2, a))))
    return b


def main():
    y = 7
    f = lambda x: x * (ap(lambda x: x * 7 + 3, y) & (y + 4)) + y
    l = build(1, y)
    return (h0(18, h1(total(build(0, y)), (0 if y < 5 else y))), y, f(h0(18, h1(total(build(0, y)), (0 if y < 5 else y)))) + f(y), total(l) + total(l))
