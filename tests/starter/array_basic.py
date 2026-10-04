def main():
    a = array_new(10, 0)
    for i in range(10):
        a = array_set(a, i, i * i)
    t = 0
    for i in range(array_len(a)):
        t = t + array_get(a, i)
    return t
