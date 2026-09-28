# Closures through the compiled runtime: factories, higher-order calls on
# top-level functions and lambdas, composition, map with a capturing
# lambda, a closure shared and applied in a loop, closures in data. Every
# value is opaque to the specializer (through an array length) so the
# closures exist at runtime.
@data
class L:
    Nil: ()
    Cons: (h, t)


def mk(k):
    return lambda x: x + k


def sq(x):
    return x * x


def twice(f, x):
    return f(f(x))


def compose(f, g):
    return lambda x: f(g(x))


def inc(x):
    return x + 1


def dbl(x):
    return x * 2


def map(f, l):
    match l:
        case Nil():
            return Nil()
        case Cons(h, t):
            return Cons(f(h), map(f, t))


def sum(l):
    match l:
        case Nil():
            return 0
        case Cons(h, t):
            return h + sum(t)


def heavy(k):
    if k < 2:
        return k
    return heavy(k - 1) + heavy(k - 2)


def mkh(k):
    return lambda x: x + heavy(k)


def loop(g, i, acc):
    if i == 0:
        return acc
    return loop(g, i - 1, acc + g(i))


def adders(n, k):
    if n == 0:
        return Nil()
    return Cons(mk(k + n), adders(n - 1, k))


def apply_all(fs, x):
    match fs:
        case Nil():
            return x
        case Cons(f, rest):
            return apply_all(rest, f(x))


def main():
    k = array_len(array_new(7, 0))
    a = mk(k)
    b = mk(k + 10)
    r1 = a(1) + b(2) + a(3)
    r2 = twice(sq, k) + twice(lambda y: y + 1, k)
    h = compose(inc, dbl)
    r3 = h(h(k))
    r4 = sum(map(lambda x: x * k, Cons(1, Cons(2, Cons(3, Nil())))))
    r5 = loop(mkh(k + 5), 50, 0)
    r6 = apply_all(adders(k, k), 1)
    return r1 + r2 * 3 + r3 * 5 + r4 * 7 + r5 + r6 * 11
