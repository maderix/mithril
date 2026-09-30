# Net values reaching compiled code, at sizes the settle worklist must not
# bound: a closure returning a 200-element list (the scan must not grow
# with the list's length) and a closure returning a 40-field tuple whose
# fields are compiled calls (under a starved budget many are pending at
# once). Values are opaque to the specializer through an array length.
@data
class L:
    Nil: ()
    Cons: (h, t)


def sq(x):
    return x * x


def build(i, n):
    if i == n:
        return Nil()
    return Cons(sq(i), build(i + 1, n))


def total(l):
    match l:
        case Nil():
            return 0
        case Cons(h, t):
            return h + total(t)


def ap(f, s, n):
    if n == 0:
        return f(s)
    return ap(f, s, n - 1)


def main():
    n = array_len(array_new(3, 0))
    long = ap(lambda y: build(y, 200), n - 3, n)
    wide = ap(lambda y: (sq(y + 0), sq(y + 1), sq(y + 2), sq(y + 3), sq(y + 4), sq(y + 5), sq(y + 6), sq(y + 7), sq(y + 8), sq(y + 9), sq(y + 10), sq(y + 11), sq(y + 12), sq(y + 13), sq(y + 14), sq(y + 15), sq(y + 16), sq(y + 17), sq(y + 18), sq(y + 19), sq(y + 20), sq(y + 21), sq(y + 22), sq(y + 23), sq(y + 24), sq(y + 25), sq(y + 26), sq(y + 27), sq(y + 28), sq(y + 29), sq(y + 30), sq(y + 31), sq(y + 32), sq(y + 33), sq(y + 34), sq(y + 35), sq(y + 36), sq(y + 37), sq(y + 38), sq(y + 39)), n, n)
    return (total(long), wide[0] + wide[39])
