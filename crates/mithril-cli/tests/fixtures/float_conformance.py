# Float conformance: every f32, f16 and f64 operation over operands from
# subnormal to overflow, zeros, infinities and NaN producers. Every backend
# must write the same bytes.

def hash(k):
    x = (k + 1) * 2654435761 & 4294967295
    x = x ^ (x >> 15)
    return (x * 2246822519) & 4294967295


def mag32(k):
    m = k % 7
    if m == 0:
        return f32(1.0e-41)
    if m == 1:
        return f32(1.0e-20)
    if m == 2:
        return f32(1.0)
    if m == 3:
        return f32(3.0e19)
    if m == 4:
        return f32(3.0e38)
    if m == 5:
        return f32(1.0e-39)
    return f32(0.0)


def op32(x, y, j):
    if j == 0:
        return x + y
    if j == 1:
        return x - y
    if j == 2:
        return x * y
    if j == 3:
        return x / y
    if j == 4:
        return sqrt(x)
    if j == 5:
        return x * y + x
    if j == 6:
        return (x * y) / (x - y)
    return x - y * y


def mag16(k):
    m = k % 7
    if m == 0:
        return f16(1.0e-7)
    if m == 1:
        return f16(0.001)
    if m == 2:
        return f16(1.0)
    if m == 3:
        return f16(30.0)
    if m == 4:
        return f16(65.0)
    if m == 5:
        return f16(1.0e-4)
    return f16(0.0)


def op16(p, q, j):
    if j == 0:
        return p + q
    if j == 1:
        return p - q
    if j == 2:
        return p * q
    if j == 3:
        return p / q
    if j == 4:
        return sqrt(p)
    if j == 5:
        return p * q + p
    if j == 6:
        return (p * q) / (p - q)
    return -(p / q)


def mag64(k):
    m = k % 7
    if m == 0:
        return 1.0e-310
    if m == 1:
        return 1.0e-200
    if m == 2:
        return 1.0
    if m == 3:
        return 1.0e200
    if m == 4:
        return 1.0e308
    if m == 5:
        return 2.0e-308
    return 0.0


def op64(x, y, j):
    if j == 0:
        return x + y
    if j == 1:
        return x - y
    if j == 2:
        return x * y
    if j == 3:
        return x / y
    if j == 4:
        return x * y + x
    if j == 5:
        return (x * y) / (x - y)
    return x - y * y


def main():
    n = array_len(array_new(420, 0))
    a = array_new(8 * n, f32(0.0))
    for c in range(8 * n):
        k = c // 8
        x = f32(hash(k) % 2001 - 1000) * mag32(k)
        y = f32(hash(k + 7) % 2001 - 1000) * mag32(k // 7)
        a = array_set(a, c, op32(x, y, c % 8))
    h = array_new(8 * n, f16(0.0))
    for c in range(8 * n):
        k = c // 8
        p = f16(hash(k + 3) % 2001 - 1000) * mag16(k)
        q = f16(hash(k + 11) % 2001 - 1000) * mag16(k // 7)
        h = array_set(h, c, op16(p, q, c % 8))
    # f64 operands: a logistic-map sequence from a run-time seed (pure f64)
    v = array_get(array_new(n, 0.3141592653589793), 0)
    seq = array_new(n + 8, v)
    for k in range(n + 8):
        v = v * 3.91 * (1.0 - v)
        seq = array_set(seq, k, v)
    b = array_new(7 * n, v)
    for d in range(7 * n):
        m = d // 7
        u = (array_get(seq, m) - 0.5) * mag64(m)
        w = (array_get(seq, m + 7) - 0.5) * mag64(m // 7)
        b = array_set(b, d, op64(u, w, d % 7))
    return (a, h, b)
