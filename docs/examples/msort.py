# Merge sort used by docs/guide.html. The length comes from an array so the
# compiler cannot fold the whole program into a constant.
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


def check(l, i, acc):
    match l:
        case Nil():
            return acc
        case Cons(h, t):
            return check(t, i + 1, (acc * 31 + h * i) & 4294967295)


def main():
    n = array_len(array_new(65536, 0))
    return check(msort(gen(n, 1, Nil())), 1, 0)
