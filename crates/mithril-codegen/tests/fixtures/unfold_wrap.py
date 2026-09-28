# compile-time unfolding: control of `rounds` is static at the call site
def key(j):
    if j == 0:
        return 7
    if j == 1:
        return 11
    if j == 2:
        return 13
    return 17


def rounds(n, j, x):
    if n == 0:
        return x
    return rounds(n - 1, j + 1, ((x * 31) ^ key(j)) & 1048575)


# dynamic control: must stay a loop
def collatz(x, steps):
    if x == 1:
        return steps
    if (x & 1) == 0:
        return collatz(x >> 1, steps + 1)
    return collatz(3 * x + 1, steps + 1)


# i56 wrapping at the boundary: products and shifts that overflow 56 bits,
# with and without a mask after them, and negative differences
def wraps(a, b):
    big = (a << 50) * (b + 3)
    masked = ((a << 40) * (b << 20)) & 16777215
    neg = (a & 255) - (b & 65535)
    hi = (a << 54) + (b << 54)
    return (big ^ masked ^ neg ^ hi) & 36028797018963967


# fork recursion over ints: a scalar function that must still split
def ftree(d, x):
    if d == 0:
        return rounds(4, 0, x)
    return (ftree(d - 1, (x * 3 + 1) & 65535) + ftree(d - 1, (x * 5 + 2) & 65535)) & 1048575


def loop(i, acc):
    if i == 0:
        return acc
    return loop(i - 1, (acc + rounds(3, 0, i) + collatz(i + 1, 0) + wraps(i, acc & 1023)) & 36028797018963967)


def main():
    return (loop(300, 5) + ftree(12, 9)) & 36028797018963967
