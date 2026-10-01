# Tail values read from arrays this function owns: read, then free.
def last(n, k):
    a = array_new(n, k)
    return array_get(a, n - 1) + array_len(a)


def pair(n, k):
    a = array_new(n, k)
    return (array_get(a, 0), array_get(a, n - 1))


def main():
    n = array_len(array_new(65536, 0))
    p = pair(n, 5)
    return last(n, 7) + p[0] * 1000 + p[1]
