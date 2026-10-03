# histogram: count n hashed keys into 8 buckets. The counters are one
# 8-tuple combined by elementwise addition, a proven fold, so the key range
# splits into chunks whose partial histograms are added together.
# Checksum: sum of bucket count * (bucket + 1), u32. Sizes: small n 1000,
# big n 2^29.


def prng(x):
    b = x ^ ((x << 13) & 4294967295)
    d = b ^ (b >> 17)
    return d ^ ((d << 5) & 4294967295)


def hadd(a, b):
    return (a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3], a[4] + b[4], a[5] + b[5], a[6] + b[6], a[7] + b[7])


def one(k):
    return (1 if k == 0 else 0, 1 if k == 1 else 0, 1 if k == 2 else 0, 1 if k == 3 else 0, 1 if k == 4 else 0, 1 if k == 5 else 0, 1 if k == 6 else 0, 1 if k == 7 else 0)


def hist(n):
    h = (0, 0, 0, 0, 0, 0, 0, 0)
    for i in range(n):
        h = hadd(h, one(prng((i + 1) * 2654435761 & 4294967295) & 7))
    return h


def main():
    h = hist(536870912)
    return (h[0] + 2 * h[1] + 3 * h[2] + 4 * h[3] + 5 * h[4] + 6 * h[5] + 7 * h[6] + 8 * h[7]) & 4294967295
