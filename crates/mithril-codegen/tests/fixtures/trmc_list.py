@data
class L:
    Nil: ()
    Cons: (h, t)


def step(x):
    return (x * 1103515245 + 12345) & 2147483647


# direct TRMC: cons onto the self call; `rest` is the tail parameter, so
# this is also a destination-passing callee
def run_of(n, x, rest):
    if n == 0:
        return rest
    y = step(x)
    return Cons(y % 100, run_of(n - 1, y, rest))


# delayed self call consumed in branches: as the cons tail, or as the
# tail parameter of run_of
def build(k, x):
    if k == 0:
        return Nil()
    rest = build(k - 1, step(x))
    m = x % 3
    if m == 0:
        return run_of(1 + x % 5, x, rest)
    if m == 1:
        return Cons(x % 7, rest)
    return Cons(x % 11, rest)


# not eligible (the self call's result is wrapped twice): must fall back
def build2(k, x):
    if k == 0:
        return Nil()
    rest = build2(k - 1, step(x))
    return Cons(x % 11, Cons(x % 13, rest))


def total(l, acc):
    match l:
        case Nil():
            return acc
        case Cons(h, t):
            return total(t, (acc * 31 + h) & 1048575)


def main():
    a = total(build(2000, 7), 0)
    b = total(build2(500, 3), 0)
    return (a * 1000003 + b) & 1048575
