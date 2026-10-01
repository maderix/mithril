# Constructor values matched in native code: consumed (owned) and lent (borrowed),
# with unused fields, a three-field cell and a nullary (unboxed) constructor.
@data
class L:
    Nil: ()
    Cons: (h, t)
    Tri: (a, b, t)


def build(n, i):
    if i == n:
        return Nil()
    if i % 3 == 0:
        return Tri(i, i * 7, build(n, i + 1))
    return Cons(i * 3 + 1, build(n, i + 1))


def total(l, acc):
    match l:
        case Nil():
            return acc
        case Cons(h, t):
            return total(t, (acc * 31 + h) & 4294967295)
        case Tri(a, b, t):
            return total(t, (acc * 17 + a) & 4294967295)


def twice(n):
    l = build(n, 0)
    return (total(l, 1) * 3 + total(l, 2)) & 4294967295


def main():
    return (total(build(5000, 0), 0) + twice(3000)) & 4294967295
