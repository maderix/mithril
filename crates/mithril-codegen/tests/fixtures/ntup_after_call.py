# a native multi-value result projected after a suspendable call: the
# boxed bridge, not locals a capture cannot hold (rustc E0425 before)
def bins(i, n):
    a0 = 0
    a1 = 0
    for j in range(n):
        h = ((i + j + 1) * 2654435761) & 4294967295
        a0 = (a0 + h) & 4294967295
        a1 = (a1 ^ h) & 4294967295
    return (a0, a1)


def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)


def g(i):
    t = bins(i, 1000 + (i & 3))
    y = fib((t[0] & 7) + 14)
    return (y + t[1]) & 4294967295


def build(d, i):
    if d == 0:
        return g(i)
    return (build(d - 1, i * 2) + build(d - 1, i * 2 + 1)) & 4294967295


def main():
    return build(8, 1)
