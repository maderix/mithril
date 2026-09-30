# Self-referential types meeting a closure result: a list of pairs and an
# array of arrays, each seeded by an untyped closure value. Type inference
# must terminate and keep the parts unknown.
def ap(f, s, n):
    if n == 0:
        return f(s)
    return ap(f, s, n - 1)


def build(acc, n):
    if n == 0:
        return acc
    return build((n, acc), n - 1)


def nest(a, n):
    if n == 0:
        return a
    return nest(array_new(1, a), n - 1)


def main():
    n = array_len(array_new(3, 0))
    x = ap(lambda y: (y, 0), n, n)
    a = ap(lambda y: array_new(1, y), n, n)
    return (build(x, n)[0], array_len(nest(a, n)))
