# CPU+GPU co-execution benchmark: one subset-sum search (the subsetsum port
# at n = 36) split into 1,024 independent sub-searches by the take/skip
# choice of its first ten weights. The sub-searches differ widely in size,
# and the CPU and the GPU are about equally fast on them, so with --coop
# both engines claim chunks of the sum: mithril run subsetsum_split.py
# --coop --threads 15. Checksum: the count, u32.


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


def walk(j, i, k, n, room):
    if i == k:
        return count(k, n, room)
    if (j >> i) & 1 == 1:
        w = weight(i)
        if w > room:
            return 0
        return walk(j, i + 1, k, n, room - w)
    return walk(j, i + 1, k, n, room)


def main():
    n = 36
    room = total_weight(n) // 3
    s = 0
    for j in range(0, 1024):
        s = (s + walk(j, 0, 10, n, room)) & 4294967295
    return s
