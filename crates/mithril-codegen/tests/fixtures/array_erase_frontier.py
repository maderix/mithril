# Array destruction must retain its unvisited suffix as one continuation.
# A copied array shares the element cells; only the last drop erases them.
@data
class Chain:
    Nil: ()
    Cons: (v, t)


def head(c):
    match c:
        case Nil():
            return 0
        case Cons(v, t):
            return v


def run(n):
    a = array_new(n, Nil())
    for j in range(n):
        a = array_set(a, j, Cons(j, Cons(j + 1, Cons(j + 2, Nil()))))
    if n == 0:
        return 0
    b = array_set(a, n - 1, Cons(n, Cons(n + 1, Nil())))
    nested = array_new(3, b)
    return head(array_get(a, n - 1)) * 31 + head(array_get(array_get(nested, 2), n - 1))


def main():
    return run(array_len(array_new(1024, 0)))
