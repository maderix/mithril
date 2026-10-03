# subsetsum: count the subsets of n hashed weights whose sum stays within a
# capacity. A backtracking search: at each weight it counts the subsets that
# skip it and, when the weight still fits, the subsets that take it. The
# two branches are independent self-calls, so the search forks; branches
# that run out of capacity end early, so their sizes vary. Weights are
# xorshift hashes of the index in [1, 65536]. Checksum: the count, u32.
# Sizes: small n 16, big n 32 (capacity a third of the total weight).


def prng(x):
    b = x ^ ((x << 13) & 4294967295)
    d = b ^ (b >> 17)
    return d ^ ((d << 5) & 4294967295)


def weight(i):
    return (prng((i + 1) * 2654435761 & 4294967295) & 65535) + 1


def total_weight(n):
    s = 0
    for i in range(n):
        s = s + weight(i)
    return s


def count(i, n, room):
    if i == n:
        return 1
    skip = count(i + 1, n, room)
    w = weight(i)
    if w > room:
        return skip
    take = count(i + 1, n, room - w)
    return (skip + take) & 4294967295


def main():
    n = 32
    return count(0, n, total_weight(n) // 3)
