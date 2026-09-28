# A persistent (functional) binary search tree map. Every insert makes a
# new version sharing most nodes with the old one; old versions stay live
# and are queried after later inserts. Irregular recursion, sharing.
@data
class M:
    E: ()
    N: (k, v, l, r)


def ins(m, k, v):
    match m:
        case E():
            return N(k, v, E(), E())
        case N(mk, mv, l, r):
            if k < mk:
                return N(mk, mv, ins(l, k, v), r)
            if k > mk:
                return N(mk, mv, l, ins(r, k, v))
            return N(k, v, l, r)


def get(m, k):
    match m:
        case E():
            return 0
        case N(mk, mv, l, r):
            if k < mk:
                return get(l, k)
            if k > mk:
                return get(r, k)
            return mv


def size(m):
    match m:
        case E():
            return 0
        case N(mk, mv, l, r):
            return 1 + size(l) + size(r)


def rnd(x):
    return (x * 1103515245 + 12345) & 2147483647


def run(n):
    m = E()
    old = E()
    x = 7
    s = 0
    for i in range(n):
        x = rnd(x)
        m = ins(m, x & 65535, i)
        if (i & 1023) == 0:
            old = m
        s = (s + get(m, (x >> 3) & 65535) + get(old, x & 65535)) & 4294967295
    return (s + size(m) * 7 + size(old)) & 4294967295


def main():
    return run(400000)
