# i56 arithmetic at the edges: wrapping add/sub/mul/shl, arithmetic right
# shift of negatives, floor division and Python modulo of negatives,
# comparisons across the sign
def mix(n, x):
    acc = x
    big = 36028797018963967
    for i in range(n):
        a = acc * 1103515245 + 12345
        b = (a << 13) ^ (a >> 7)
        c = b - big
        d = c // 97 if (i & 1) == 0 else c // (0 - 89)
        e = c % 1000003 if (i & 2) == 0 else c % (0 - 1000033)
        f = (e >> 3) + (d >> 60) + (i << 50)
        g = (0 - f) if f < 0 else f + 1
        acc = (acc ^ g) + (1 if a > b else 0) - (1 if c <= d else 0)
    return acc


# an add/min chain over two rolling rows (the edit-distance shape)
def dp(n, s):
    prev = array_new(n + 1, 0)
    cur = array_new(n + 1, 0)
    for k in range(n + 1):
        prev = array_set(prev, k, k * s - 1000)
    for i in range(n):
        cur = array_set(cur, 0, i + 1 - s)
        for j in range(n):
            up = array_get(prev, j + 1) + 1
            left = array_get(cur, j) + 1
            diag = array_get(prev, j) + ((i * j + s) % 3) - 1
            v = up if up < left else left
            v = v if v < diag else diag
            cur = array_set(cur, j + 1, v)
        t = prev
        prev = cur
        cur = t
    return array_get(prev, n) * 7 + array_len(prev)


# mutual recursion through a loop helper (must not split representations)
def walk(n, acc):
    if n <= 0:
        return acc
    total = acc
    i = 0
    while i < 3:
        total = walk(n - 1 - i, total) + i - (n >> 1)
        i = i + 1
    return total


def work(k):
    return mix(40, k * 77 - 5000) + dp(12, k) + walk(6 + (k & 3), k)


def batch(d, k):
    if d == 0:
        return work(k)
    return batch(d - 1, k * 2) + batch(d - 1, k * 2 + 1)


def main():
    return batch(3, 5)
