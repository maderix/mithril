def main():
    n = 100
    p = array_new(n, 1)
    p = array_set(p, 0, 0)
    p = array_set(p, 1, 0)
    for i in range(2, n):
        if array_get(p, i) == 1:
            j = i * i
            while j < n:
                p = array_set(p, j, 0)
                j = j + i
    c = 0
    for i in range(n):
        c = c + array_get(p, i)
    return c
