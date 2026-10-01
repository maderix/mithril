def shade(x, y):
    h = x * 73856093 ^ y * 19349663
    k = 0
    while k < 400:
        h = (h * 1103515245 + 12345) & 2147483647
        k = k + 1
    return h & 16777215


def main():
    w = array_len(array_new(128, 0))
    img = array_new(w * w, 0)
    for y in range(w):
        for x in range(w):
            img = array_set(img, y * w + x, shade(x, y))
    s = 0
    for i in range(w * w):
        s = (s * 31 + array_get(img, i)) & 4294967295
    return s
