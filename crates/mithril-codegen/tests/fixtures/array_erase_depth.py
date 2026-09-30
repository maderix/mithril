# Only 41 elements, but more than 32 nested erasure frames.
def ap(f, s, n):
    if n == 0:
        return f(s)
    return ap(f, s, n - 1)


def nest(a, n):
    if n == 0:
        return a
    return nest(array_new(1, a), n - 1)


def main():
    n = array_len(array_new(40, 0))
    a = ap(lambda y: array_new(1, y), n, n)
    return array_len(nest(a, n))
