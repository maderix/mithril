# A closure result (untyped) passed where an int also goes: the receiving
# function must not read it as an int.
def pick(x, n):
    if n == 0:
        return x
    return pick(x, n - 1)


def ap(f, s, n):
    if n == 0:
        return f(s)
    return ap(f, s, n - 1)


def main():
    n = array_len(array_new(3, 0))
    m = array_len(array_new(3, 0))
    s = array_get(array_new(n, 2.5), 0)
    g = ap(lambda y: y * s, s, n)
    return (pick(g, n), m)
