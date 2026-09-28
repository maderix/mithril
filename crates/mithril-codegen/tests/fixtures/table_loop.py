@data
class Chain:
    CNil: ()
    CCons: (k, t)


@data
class Arr:
    Lf: (v,)
    Br: (l, r)


# a full tree of depth d with every leaf v
def mk(d, v):
    if d == 0:
        return Lf(v)
    return Br(mk(d - 1, v), mk(d - 1, v))


def aget(t, i):
    match t:
        case Lf(v):
            return v
        case Br(l, r):
            if (i & 1) == 0:
                return aget(l, i >> 1)
            return aget(r, i >> 1)


def aset(t, i, v):
    match t:
        case Lf(w):
            return Lf(v)
        case Br(l, r):
            if (i & 1) == 0:
                return Br(aset(l, i >> 1, v), r)
            return Br(l, aset(r, i >> 1, v))


# xorshift: x ^= x<<13; x ^= x>>17; x ^= x<<5
def prng(x):
    b = x ^ ((x << 13) & 4294967295)
    d = b ^ (b >> 17)
    return d ^ ((d << 5) & 4294967295)
def chas(c, k):
    match c:
        case CNil():
            return 0
        case CCons(h, t):
            if h == k:
                return 1
            return chas(t, k)


# a chain's length
def clen(c, acc):
    match c:
        case CNil():
            return acc
        case CCons(h, t):
            return clen(t, acc + 1)



def walk(t):
    match t:
        case Lf(v):
            return clen(v, 0)
        case Br(l, r):
            return (walk(l) * 3 + walk(r)) & 1048575

def main():
    s = 0
    for i in range(120):
        a = mk(8, CNil())
        for j in range(i % 20 + 1):
            a = aset(a, j & 7, CCons(j, CNil()))
        s = s + clen(aget(a, 3), 0)
    return s
