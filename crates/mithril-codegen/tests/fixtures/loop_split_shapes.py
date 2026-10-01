# Loops the split pass rewrites: the result must be the sequential one.
def f(i, k):
    return (i * 2654435761 + k) & 1048575


def last_write(n):
    a = array_new(4, 0)
    for i in range(n):
        a = array_set(a, i & 3, f(i, 1))
    return array_get(a, 0) * 7 + array_get(a, 3)


def ordered(n):
    s = 0
    for i in range(n):
        s = (s * 31 + f(i, 2)) & 4294967295
    return s


def from_to(a, b):
    s = 0
    for i in range(a, b):
        s = (s * 17 + f(i, 3)) & 4294967295
    return s


def floats(n):
    s = 0.0
    for i in range(n):
        s = s * 0.5 + f32(f(i, 4))
    return int(s)


def tuple_state(n):
    p = (0, 1)
    for i in range(n):
        p = (p[1], (p[0] * 3 + f(i, 5)) & 1048575)
    return p[0] + p[1]


def main():
    r = last_write(37) + ordered(100) + from_to(5, 41) + floats(50) + tuple_state(30)
    return r + ordered(0) + ordered(-3) + from_to(9, 4) + ordered(1)
