@data
class T:
    L: (x, n, k)
    N: (a, b)


def gen(d, h):
    if d == 0:
        return L(h & 255, (h & 7) + 1, h & 3)
    return N(gen(d - 1, (h * 3 + 1) & 65535), gen(d - 1, (h * 5 + 2) & 65535))


# a child of a tree (the L arm is an unreachable filler); the net inlines
# these into `pick`, whose tree is then only read: lent
def lft(t):
    match t:
        case L(x, n, k):
            return L(x, n, k)
        case N(a, b):
            return a


def rgt(t):
    match t:
        case L(x, n, k):
            return L(x, n, k)
        case N(a, b):
            return b


# a leaf read of a lent tree
def leaf(s, old):
    match s:
        case L(x, n, k):
            if n == 0:
                return old
            return ((x // n) + k) & 65535
        case N(a, b):
            return old


# reads the lent tree through let-bound children, one of them unused on
# a branch, and consumes the picks by matching on them
def pick(t, c0, c1, c2, c3):
    match t:
        case L(x, n, k):
            return (c0, c1, c2, c3)
        case N(a, b):
            p = lft(a)
            q = rgt(a)
            r = lft(b)
            s = rgt(b)
            if c0 == 0:
                return (leaf(p, c0), leaf(q, c1), c2, c3)
            return (leaf(p, c0), leaf(q, c1), leaf(r, c2), leaf(s, c3))


# the same tree picked many times: shared, so every reader lends it
def rounds(k, t, c0, c1, c2, c3, acc):
    if k == 0:
        return acc
    v = pick(t, c0, c1, c2, c3)
    # allocation churn between rounds: a wrongly freed cell of `t` is
    # reused here and the next pick reads garbage
    w = leaf(lft(gen(2, k)), 0)
    return rounds(k - 1, t, v[1], v[2], v[3], (v[0] + k) & 65535, (acc + v[0] + v[1] + v[2] + v[3] + w) & 1048575)


def batch(d, s):
    if d == 0:
        return rounds(24, gen(2, s), 0, 1, 2, 3, 0)
    return (batch(d - 1, (s * 3 + 1) & 65535) + batch(d - 1, (s * 5 + 2) & 65535)) & 1048575


def main():
    return batch(12, 11)
