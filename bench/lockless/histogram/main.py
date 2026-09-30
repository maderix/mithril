# histogram: 2^S keys hashed into 2^16 bins, counted without shared state.
# Key i is the top 16 bits of a xorshift hash of (i + 1) * 2654435761. The
# count is a fold over the index range: a leaf counts 2^16 consecutive keys
# into its own fresh array of bins (as many keys as bins, so merging costs
# no more than counting), and each inner step forks both halves and adds
# the right counts into the left array, bin by bin. No array is ever
# written by two tasks: each is owned by the one call that built it.
# Checksum: sum over bins i of mix(i, count_i), u32 (position-weighted, so
# it depends on which bin holds which count).
# Sizes (S): small 20 expect 3454372420; big 24 expect 3310280667.


def prng(x):
    b = x ^ ((x << 13) & 4294967295)
    d = b ^ (b >> 17)
    return d ^ ((d << 5) & 4294967295)


# a position-weighted mix of entry i holding v
def mix(i, v):
    return prng((((i + 1) * 2654435761) & 4294967295) ^ ((v * 2246822519) & 4294967295))


def key(i):
    return prng(((i + 1) * 2654435761) & 4294967295) >> 16


# a leaf: n keys from lo counted into fresh bins
def count(lo, n):
    a = array_new(65536, 0)
    for i in range(n):
        b = key(lo + i)
        a = array_set(a, b, array_get(a, b) + 1)
    return a


# the right half's counts added into the left half's bins
def add_into(a, b):
    for i in range(65536):
        a = array_set(a, i, array_get(a, i) + array_get(b, i))
    return a


# the fold over 2^k keys from lo: both halves fork
def hist(lo, k):
    if k <= 16:
        return count(lo, 1 << k)
    a = hist(lo, k - 1)
    b = hist(lo + (1 << (k - 1)), k - 1)
    return add_into(a, b)


def digest(a):
    h = 0
    for i in range(65536):
        h = (h + mix(i, array_get(a, i))) & 4294967295
    return h


def size():
    return 24  # SIZE


def main():
    return digest(hist(0, size()))
