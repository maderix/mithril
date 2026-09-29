# boxed float comparisons at runtime (the compared values are owned and
# released by the compare: it must read them first), including NaN, which
# only `!=` holds for (IEEE 754, as in the oracle and the reducer)
def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)


def count_below(x, n, acc):
    if n == 0:
        return acc
    z = x / 3.0
    if z < 1.0:
        return count_below(x + 1.0, n - 1, acc + 1)
    return count_below(x + 1.0, n - 1, acc)


def nans(x, n, acc):
    if n == 0:
        return acc
    z = x / 0.0 - x / 0.0
    k = acc
    if z != z:
        k = k + 1
    if z < 1.0:
        k = k + 100
    if z == z:
        k = k + 1000
    return nans(x + 1.0, n - 1, k)


def main():
    n = fib(20) & 15
    return count_below(0.0, n, 0) * 100000 + nans(1.0, n, 0)
