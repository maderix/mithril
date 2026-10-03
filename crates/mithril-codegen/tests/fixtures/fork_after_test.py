# a search whose second branch waits on a test that does not depend on the
# first: read back with the test first, both calls run as parallel tasks;
# and a deep plain recursion that native code runs on the hardware stack
def weight(i):
    return ((i * 2654435761 + 7) & 1023) + 1


def count(i, n, room):
    if i == n:
        return 1
    skip = count(i + 1, n, room)
    w = weight(i)
    if w > room:
        return skip
    take = count(i + 1, n, room - w)
    return (skip + take) & 4294967295


def depth(n):
    if n == 0:
        return 0
    return (depth(n - 1) * 3 + n) & 1048575


def main():
    n = array_len(array_new(19, 0))
    d = array_len(array_new(200000, 0))
    return (count(0, n, 2600), depth(d))
