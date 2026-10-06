# Range launches the Metal GPU runs: wrapping and 32-bit int sums, a fill
# that reads a borrowed array, binary32 work that makes subnormals, guarded
# recursion, and recursion too deep for the device's stack (its request
# falls back to the CPU). Every backend must write the same bytes.

def mix(x):
    x = ((x >> 16) ^ x) * 73244475 & 4294967295
    return (x >> 16) ^ x


def wide(i):
    # wraps past 2^63 when summed
    return mix(i) * 2862933555777941757


def low(i):
    return (mix(i) * 2654435761) & 4294967295


def tiny(i):
    # products and quotients far below the least normal binary32
    x = f32(mix(i) % 1000 + 1) * 1e-30
    y = x * 1e-12 / f32(i % 7 + 1)
    return int(y * 1e45) + int(sqrt(x * 1e-9) * 1e30)


def walk(n, k):
    # guarded recursion: not a tail call
    if n <= 0:
        return k
    return walk(n - 1, mix(k)) + 1


def table():
    t = array_new(256, 0)
    for i in range(256):
        t = array_set(t, i, mix(i) % 997)
    return t


def main():
    n = 1 << 15
    s = 0
    for i in range(n):
        s = s + wide(i)
    m = 0
    for i in range(n):
        m = (m + low(i)) & 4294967295
    t = table()
    img = array_new(n, 0)
    for i in range(n):
        img = array_set(img, i, array_get(t, i % 256) * 3 + tiny(i) + walk(i % 9, i) % 1000)
    deep = 0
    for i in range(n // 64):
        deep = deep + walk(1500 + i % 3, i) % 7
    return (s, m, img, deep)
