@data
class T:
    Leaf: (v,)
    Node: (l, r)


def mk(d, x):
    if d == 0:
        return Leaf(x & 1023)
    return Node(mk(d - 1, (x * 3 + 1) & 65535), mk(d - 1, (x * 5 + 2) & 65535))


# consumes t; the second recursive call is the TRMC site
def swap_add(t, k):
    match t:
        case Leaf(v):
            return Leaf((v + k) & 1023)
        case Node(a, b):
            return Node(swap_add(b, k + 1), swap_add(a, k + 2))


def sumt(t):
    match t:
        case Leaf(v):
            return v
        case Node(a, b):
            return (sumt(a) * 7 + sumt(b)) & 1048575


def main():
    return sumt(swap_add(mk(12, 1), 0))
