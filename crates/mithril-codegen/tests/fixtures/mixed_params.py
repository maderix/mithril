# A parameter used at conflicting types (a tuple and an int; a runtime
# float and an int): its function must not read a non-int as an int. And
# an int result feeding a mixed consumer (the tree of results below) must
# not make the int arithmetic that produced it lose its type.
def pick(x, n):
    if n == 0:
        return x
    return pick(x, n - 1)


def pick2(x, n):
    if n == 0:
        return x
    return pick2(x, n - 1)


def leaf(i):
    return ((i * 7) >> 1) ^ 5


def tree(lo, n):
    if n == 1:
        return leaf(lo)
    h = n // 2
    return (tree(lo, h), tree(lo + h, n - h))


def main():
    n = array_len(array_new(3, 0))
    g = array_get(array_new(n, 2.5), 0)
    return (pick((n, 5), n), pick(n, n), pick2(g, n), pick2(n, n), tree(0, 5))
