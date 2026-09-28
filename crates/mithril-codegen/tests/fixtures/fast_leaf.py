@data
class T:
    Lf: (v,)
    Nil: ()
    Nd: (l, r)


def mk(d, x):
    if d == 0:
        if (x & 3) == 0:
            return Nil()
        return Lf(x & 255)
    return Nd(mk(d - 1, (x * 3 + 1) & 65535), mk(d - 1, (x * 5 + 2) & 65535))


# base cases on t are call-free; u is owned and must be dropped there
def zipsum(t, u, k):
    match t:
        case Lf(v):
            return (v + k) & 1048575
        case Nil():
            return k
        case Nd(a, b):
            match u:
                case Nd(c, d):
                    return (zipsum(a, c, k + 1) + zipsum(b, d, k + 2)) & 1048575
                case Lf(w):
                    return w
                case Nil():
                    return 0


# tree to tree: the leaf arms build cell-free values
def inc(t):
    match t:
        case Lf(v):
            return Lf((v + 1) & 255)
        case Nil():
            return Nil()
        case Nd(a, b):
            return Nd(inc(a), inc(b))


def main():
    return zipsum(inc(mk(10, 3)), mk(11, 5), 0)
