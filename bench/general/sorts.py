# Mergesort and quicksort on cons lists; partition returns a tuple; the
# input list is sorted twice (shared).
@data
class L:
    Nil: ()
    Cons: (h, t)


def gen(n, x, acc):
    if n == 0:
        return acc
    y = (x * 1103515245 + 12345) & 2147483647
    return gen(n - 1, y, Cons(y & 1048575, acc))


def split(l, a, b):
    match l:
        case Nil():
            return (a, b)
        case Cons(h, t):
            return split(t, b, Cons(h, a))


def merge(a, b):
    match a:
        case Nil():
            return b
        case Cons(x, xs):
            match b:
                case Nil():
                    return a
                case Cons(y, ys):
                    if x <= y:
                        return Cons(x, merge(xs, b))
                    return Cons(y, merge(a, ys))


def msort(l):
    match l:
        case Nil():
            return l
        case Cons(h, t):
            match t:
                case Nil():
                    return l
                case Cons(h2, t2):
                    p = split(l, Nil(), Nil())
                    return merge(msort(p[0]), msort(p[1]))


def part(l, piv, lo, hi):
    match l:
        case Nil():
            return (lo, hi)
        case Cons(h, t):
            if h < piv:
                return part(t, piv, Cons(h, lo), hi)
            return part(t, piv, lo, Cons(h, hi))


def app(a, b):
    match a:
        case Nil():
            return b
        case Cons(h, t):
            return Cons(h, app(t, b))


def qsort(l):
    match l:
        case Nil():
            return l
        case Cons(h, t):
            p = part(t, h, Nil(), Nil())
            return app(qsort(p[0]), Cons(h, qsort(p[1])))


def check(l, i, acc):
    match l:
        case Nil():
            return acc
        case Cons(h, t):
            return check(t, i + 1, (acc * 31 + h * i) & 4294967295)


def run(n):
    s = 0
    for r in range(8):
        l = gen(n, r + 1, Nil())
        s = (s + check(msort(l), 1, 0) + check(qsort(l), 1, 0)) & 4294967295
    return s


def main():
    return run(60000)
