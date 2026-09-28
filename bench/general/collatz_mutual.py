# Irregular mutual recursion: collatz step counts through even/odd
# functions that call each other, plus a recursive digit-sum reduction.
def ev_step(n, c):
    if n == 1:
        return c
    if (n & 1) == 0:
        return ev_step(n >> 1, c + 1)
    return od_step(n, c)


def od_step(n, c):
    return ev_step(3 * n + 1, c + 1)


def dsum(n):
    if n < 10:
        return n
    return dsum(n // 10) + n % 10


def run(n):
    s = 0
    best = 0
    for i in range(1, n):
        c = ev_step(i, 0)
        if c > best:
            best = c
        s = (s + c * dsum(i)) & 4294967295
    return (s + best) & 4294967295


def main():
    return run(3000000)
