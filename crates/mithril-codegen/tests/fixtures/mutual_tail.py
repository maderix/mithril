# even/odd mutual tail recursion, hundreds of thousands of steps deep: tail inlining
# turns it into one loop (no stack growth)
def ev(n, c):
    if n == 0:
        return c
    return od(n - 1, c + 2)


def od(n, c):
    if n == 0:
        return c + 1
    return ev(n - 1, (c * 3) & 1048575)


def main():
    s = 0
    for k in range(4):
        s = (s + ev(300000 + k, k)) & 4294967295
    return s
