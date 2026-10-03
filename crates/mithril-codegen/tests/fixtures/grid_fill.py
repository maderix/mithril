# Row-major nested fills (index y * m + x, either operand order) run as one
# flat index fill each. The bounds come from array lengths, so compile-time
# reduction cannot fold them; the empty cases would write out of bounds if a
# row of a negative or empty grid ran.


def dim(k):
    return array_len(array_new(k, 0))


def checksum(a):
    s = 0
    for i in range(array_len(a)):
        s = (s * 31 + array_get(a, i)) & 4294967295
    return s


def grid(n, m):
    a = array_new(n * m, 7)
    for y in range(n):
        for x in range(m):
            t = x * 31 + y
            a = array_set(a, y * m + x, t * t % 1000003)
    return a


def swapped(n, m):
    a = array_new(n * m, 7)
    for y in range(n):
        for x in range(m):
            a = array_set(a, x + m * y, x * 3 + y * 1000)
    return a


def no_cells(n, m):
    a = array_new(4, 5)
    for y in range(n):
        for x in range(m):
            a = array_set(a, y * m + x, 1)
    return a


def main():
    n = dim(45)
    m = dim(37)
    z = dim(0)
    e = (checksum(no_cells(n, z)), checksum(no_cells(z, m)), checksum(no_cells(z - 2, z - 3)), checksum(no_cells(n, z - 1)))
    return (checksum(grid(n, m)), checksum(swapped(m, n)), e, array_len(grid(z, m)))
