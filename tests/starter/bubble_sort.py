def sort(a):
    n = array_len(a)
    for i in range(n):
        for j in range(n - 1 - i):
            if array_get(a, j) > array_get(a, j + 1):
                x = array_get(a, j)
                a = array_set(a, j, array_get(a, j + 1))
                a = array_set(a, j + 1, x)
    return a


def main():
    a = array_new(8, 0)
    vals = (5, 2, 9, 1, 7, 3, 8, 6)
    for i in range(8):
        a = array_set(a, i, (i * 37 + 11) % 17)
    a = sort(a)
    t = 0
    for i in range(8):
        t = t * 10 + array_get(a, i) % 10
    return t
