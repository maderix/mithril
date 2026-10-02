@data
class Tree:
    Leaf: (v,)
    Pair: (l, r)
    Offset: (t, n)


def build(n, x):
    if n == 0:
        return Leaf(x)
    if n % 2 == 0:
        t = build(n - 1, x - 7)
        return Pair(t, t)
    return Pair(build(n - 1, x + 11), Offset(Leaf(x), n))


def chain(n):
    if n == 0:
        return Leaf(-1099511627776)
    return Pair(chain(n - 1), Offset(Leaf(n), n))


def walk(t, x):
    match t:
        case Leaf(v):
            return (v + x, 1)
        case Pair(l, r):
            a = walk(l, x)
            b = walk(r, x)
            return (a[0] * 3 - b[0], a[1] + b[1] + 1)
        case Offset(t, n):
            a = walk(t, x + n)
            return (a[0] - n, a[1] + 1)


def main():
    n = array_len(array_new(6, 0))
    t = build(n, -1099511627776)
    a = walk(t, -17)
    b = walk(t, 4294967295)
    return a[0] + b[0] + a[1] + b[1]
