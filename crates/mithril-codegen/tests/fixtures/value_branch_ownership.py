@data
class L:
    Nil: ()
    Cons: (h, t)


def len_(l, n):
    match l:
        case Nil():
            return n
        case Cons(h, t):
            return len_(t, n + 1)


# value-position branches whose arms use (or skip) outer owned values, in
# long loops: arrays (a snapshot kept across writes) and lists (a kept
# version or a fresh one, picked by an if expression and by a match)
def arrays(n):
    a = array_new(64, 1)
    snap = a
    s = 0
    for i in range(n):
        j = (i * 37) & 63
        a = array_set(a, j, (array_get(a, (j + 1) & 63) + i) & 65535)
        snap = a if (i & 7) == 0 else snap
        s = (s + array_get(snap, j)) & 4294967295
    return s


def lists(n):
    keep = Cons(1, Nil())
    cur = Nil()
    s = 0
    for i in range(n):
        cur = Cons(i & 255, cur) if (i & 3) != 0 else Cons(i, Nil())
        keep = cur if (i & 15) == 0 else keep
        m = len_(keep, 0)
        pick = 0
        match cur:
            case Nil():
                pick = 0
            case Cons(h, t):
                pick = h + len_(t, 0)
        s = (s + m + pick) & 4294967295
    return s


def main():
    return (arrays(200000) + lists(200000)) & 4294967295
