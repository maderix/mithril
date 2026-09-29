# Tuples used whole in native code: a tuple parameter passed on and
# returned as it is, a tuple variable aliased and joined, widths known
# only from type inference (pick never projects its tuple parameters).
def pick(a, b, c):
    if c < 0:
        return a
    return b


def swap(p):
    return (p[1], p[0])


def step(p, q):
    s = pick(p, q, p[0] - q[0])
    t = swap(s)
    u = t
    return u if u[0] > 3 else s


def walk(n):
    acc = (0, 1)
    for i in range(n):
        acc = step(acc, (i & 7, (i * 3) & 255))
    return acc[0] * 1000 + acc[1]


def main():
    return walk(array_len(array_new(1000, 0)))
