@data
class P:
    Pl: (a, b, c)
    Pn: (l, r)


def prng(x):
    b = (x ^ (x << 13)) & 4294967295
    d = b ^ (b >> 17)
    return (d ^ (d << 5)) & 4294967295


# data-dependent if/elif on the loop's last statement: if-converted; the
# masked arithmetic runs in 32 bits (products and differences that leave
# 32 bits and go negative are exercised)
def bins(i, n):
    a0 = 0
    a1 = 0
    a2 = 0
    s = 0
    for j in range(n):
        h = prng(((i + j + 1) * 2654435761) & 4294967295)
        k = h % 3
        d = ((h & 65535) - (h >> 16)) & 4294967295
        if k == 0:
            a0 = (a0 + h * 3) & 4294967295
        elif k == 1:
            a1 = (a1 + d) & 4294967295
        else:
            a2 = (a2 + (h >> 7)) & 4294967295
        s = (s * 31 + k) & 4294967295
    return (a0, a1, a2, s)


# dive code calling the scalar loop and projecting its tuple result
def leaf(i):
    t = bins(i, 37)
    return Pl((t[0] + t[3]) & 4294967295, t[1], t[2])


def build(d, i):
    if d == 0:
        return leaf(i)
    return Pn(build(d - 1, i * 2), build(d - 1, i * 2 + 1))


# the loop picks a subtree by a bit: a select between boxed values would
# need reference copies of both, so this branch must stay a branch
def pick(t, i):
    match t:
        case Pl(a, b, c):
            return (a + i) & 4294967295
        case Pn(l, r):
            if (i & 1) == 0:
                return pick(l, i >> 1)
            return pick(r, i >> 1)


def fold(t):
    match t:
        case Pl(a, b, c):
            return (a ^ (b * 7) ^ (c * 13)) & 4294967295
        case Pn(l, r):
            return (fold(l) * 3 + fold(r)) & 4294967295


def main():
    t = build(9, 1)
    return (fold(t) + pick(t, 300) + pick(t, 77)) & 4294967295
