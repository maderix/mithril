# Probe (absorb front-core): a closure reading two shared lists under a conditional.
# Correct: (61, 60, 150)  -- by hand: r = hd(xs) + hd(y) + 1 = 30 + 30 + 1 = 61,
# sum(y) = 60, sum(build(5)) = 150; the reference interpreter agrees.

@data
class L:
    Nil: ()
    Cons: (h, t)

def ap(f, v, n):
    if n == 0:
        return f(v)
    return ap(f, v, n - 1)

def keep(x, n):
    if n == 0:
        return x
    return keep(x, n - 1)

def build(n):
    if n == 0:
        return Nil()
    return Cons(n * 10, build(n - 1))

def sum(l):
    match l:
        case Nil:
            return 0
        case Cons(h, t):
            return h + sum(t)

def hd(l):
    match l:
        case Nil:
            return 0 - 1
        case Cons(h, t):
            return h

def main():
    n = array_len(array_new(3, 0))
    xs = build(n)
    y = keep(xs, n)
    r = ap(lambda z: hd(xs) + hd(y) + z if z > 0 else z, 1, n)
    q = build(n + 2)
    return (r, sum(y), sum(q))
