# components: connected components of an undirected random graph of 2^S
# nodes and 2^S edges; edge e joins the xorshift hashes of 2e + 1 and
# 2e + 2, masked to S bits. Parallel label propagation to a fixpoint, with
# no union-find and nothing updated in place by more than one task. The
# adjacency is an immutable trie over the node bits (a fold over the edge
# range; both halves fork and merge). Every node starts labelled with its
# own id. A round reads the labels array (shared, read only) in a fold over
# the adjacency trie (both subtrees fork) and proposes, for each node v,
# min over x in {v} and v's neighbours of lab[lab[x]]; the round's changes
# come back as a tree and are written into the array once the fold is
# done. A label is always a node of the same component and never above the
# node's id, so the fixpoint labels every node with its component's
# minimum id.
# Checksum: (components, sum over nodes v of mix(v, label_v)), u32.
# Sizes (S): small 12 expect (642, 2368481500); big 20 expect
# (154830, 2981158111).


@data
class List:
    Nil: ()
    Cons: (h, t)


@data
class Adj:
    Free: ()
    Leaf: (ns,)
    Node: (l, r)


@data
class Upd:
    Same: ()
    Set: (v, l)
    Both: (a, b)


def prng(x):
    b = x ^ ((x << 13) & 4294967295)
    d = b ^ (b >> 17)
    return d ^ ((d << 5) & 4294967295)


def mix(i, v):
    return prng((((i + 1) * 2654435761) & 4294967295) ^ ((v * 2246822519) & 4294967295))


def end(x, lg):
    return prng(((x + 1) * 2654435761) & 4294967295) & ((1 << lg) - 1)


def append(a, b):
    match a:
        case Nil():
            return b
        case Cons(h, t):
            return Cons(h, append(t, b))


# neighbour y added to node x's list in the trie t (d bits left to spell)
def add(t, x, y, d):
    if d == 0:
        match t:
            case Free():
                return Leaf(Cons(y, Nil()))
            case Leaf(ns):
                return Leaf(Cons(y, ns))
            case Node(l, r):
                return Node(l, r)
    b = (x >> (d - 1)) & 1
    match t:
        case Free():
            if b == 0:
                return Node(add(Free(), x, y, d - 1), Free())
            return Node(Free(), add(Free(), x, y, d - 1))
        case Leaf(ns):
            return Leaf(ns)
        case Node(l, r):
            if b == 0:
                return Node(add(l, x, y, d - 1), r)
            return Node(l, add(r, x, y, d - 1))


def merge(a, b):
    match a:
        case Free():
            return b
        case Leaf(na):
            match b:
                case Free():
                    return Leaf(na)
                case Leaf(nb):
                    return Leaf(append(na, nb))
                case Node(b0, b1):
                    return Node(b0, b1)
        case Node(a0, a1):
            match b:
                case Free():
                    return Node(a0, a1)
                case Leaf(nb):
                    return Node(a0, a1)
                case Node(b0, b1):
                    return Node(merge(a0, b0), merge(a1, b1))


# the adjacency of edges lo .. lo + 2^k: both halves fork, then merge
def build(lo, k, lg):
    if k == 0:
        x = end(2 * lo, lg)
        y = end(2 * lo + 1, lg)
        return add(add(Free(), x, y, lg), y, x, lg)
    a = build(lo, k - 1, lg)
    b = build(lo + (1 << (k - 1)), k - 1, lg)
    return merge(a, b)


def least(a, b):
    if b < a:
        return b
    return a


def jump(lab, x):
    return array_get(lab, array_get(lab, x))


def best(ns, lab, m):
    match ns:
        case Nil():
            return m
        case Cons(u, t):
            return best(t, lab, least(m, jump(lab, u)))


def join(a, b):
    match a:
        case Same():
            return b
        case Set(v, l):
            match b:
                case Same():
                    return Set(v, l)
                case Set(w, k):
                    return Both(Set(v, l), Set(w, k))
                case Both(b0, b1):
                    return Both(Set(v, l), Both(b0, b1))
        case Both(a0, a1):
            match b:
                case Same():
                    return Both(a0, a1)
                case Set(w, k):
                    return Both(Both(a0, a1), Set(w, k))
                case Both(b0, b1):
                    return Both(Both(a0, a1), Both(b0, b1))


# one round's proposals for the nodes of subtree t at prefix p: both
# subtrees fork; lab is only read
def relax(t, p, lab):
    match t:
        case Free():
            return Same()
        case Leaf(ns):
            m = best(ns, lab, jump(lab, p))
            if m < array_get(lab, p):
                return Set(p, m)
            return Same()
        case Node(l, r):
            a = relax(l, p * 2, lab)
            b = relax(r, p * 2 + 1, lab)
            return join(a, b)


def apply(u, lab):
    match u:
        case Same():
            return lab
        case Set(v, l):
            return array_set(lab, v, l)
        case Both(a, b):
            return apply(b, apply(a, lab))


def changed(u):
    match u:
        case Same():
            return 0
        case Set(v, l):
            return 1
        case Both(a, b):
            return 1


def rounds(adj, lab):
    u = relax(adj, 0, lab)
    if changed(u) == 0:
        return lab
    return rounds(adj, apply(u, lab))


def digest(lab, n):
    comps = 0
    h = 0
    for v in range(n):
        l = array_get(lab, v)
        if l == v:
            comps = comps + 1
        h = (h + mix(v, l)) & 4294967295
    return (comps, h)


def size():
    return 20  # SIZE


def main():
    lg = size()
    n = 1 << lg
    lab = array_new(n, 0)
    for v in range(n):
        lab = array_set(lab, v, v)
    return digest(rounds(build(0, lg, lg), lab), n)
