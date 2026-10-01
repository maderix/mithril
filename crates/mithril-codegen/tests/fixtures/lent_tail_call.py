# Callees in tail position borrow `a`: it must outlive the call.
def checksum(n, k):
    a = array_new(n, k)
    s = 0
    for i in range(n):
        s = (s * 31 + array_get(a, i)) & 4294967295
    return s


def ends(a):
    return (array_get(a, 0), array_get(a, array_len(a) - 1))


def fresh_ends(n, k):
    a = array_new(n, k)
    return ends(a)


def main():
    n = array_len(array_new(65536, 0))
    e = fresh_ends(n, 9)
    return checksum(n, 3) + e[0] * 10 + e[1]
