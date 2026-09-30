# bfs: level-synchronous breadth-first search from node 0 on a directed
# random graph of 2^S nodes, out-degree 8: neighbour j of node v is a
# xorshift hash of v * 8 + j, masked to S bits. Level sets are values: a
# set of nodes is a binary trie over the node bits (MSB at the root). One
# level expands the frontier by a fold over its trie (both subtrees fork;
# a member contributes the set of its 8 neighbours; sets merge by union),
# removes the nodes already seen (a trie difference, both sides forking)
# and adds the rest to the seen set. Nothing is marked in place, so no
# visited flag is ever raced for.
# Checksum: (nodes reached, sum of their levels).
# Sizes (S): small 12 expect (4096, 17156); big 20 expect
# (1048371, 7181108).


@data
class Set:
    Emp: ()
    One: ()
    Node: (l, r)


def prng(x):
    b = x ^ ((x << 13) & 4294967295)
    d = b ^ (b >> 17)
    return d ^ ((d << 5) & 4294967295)


def nbr(v, j, lg):
    return prng((((v * 8 + j + 1) * 2654435761) & 4294967295) ^ 2246822519) & ((1 << lg) - 1)


# x added to the set t whose members have d bits left to spell
def insert(t, x, d):
    if d == 0:
        return One()
    b = (x >> (d - 1)) & 1
    match t:
        case Emp():
            if b == 0:
                return Node(insert(Emp(), x, d - 1), Emp())
            return Node(Emp(), insert(Emp(), x, d - 1))
        case One():
            return One()
        case Node(l, r):
            if b == 0:
                return Node(insert(l, x, d - 1), r)
            return Node(l, insert(r, x, d - 1))


def union(a, b):
    match a:
        case Emp():
            return b
        case One():
            return One()
        case Node(a0, a1):
            match b:
                case Emp():
                    return Node(a0, a1)
                case One():
                    return One()
                case Node(b0, b1):
                    return Node(union(a0, b0), union(a1, b1))


# a node of two subsets, empty when both are
def node(l, r):
    match l:
        case Emp():
            match r:
                case Emp():
                    return Emp()
                case One():
                    return Node(Emp(), One())
                case Node(r0, r1):
                    return Node(Emp(), Node(r0, r1))
        case One():
            return Node(One(), r)
        case Node(l0, l1):
            return Node(Node(l0, l1), r)


# the members of a not in b
def diff(a, b):
    match a:
        case Emp():
            return Emp()
        case One():
            match b:
                case Emp():
                    return One()
                case One():
                    return Emp()
                case Node(b0, b1):
                    return One()
        case Node(a0, a1):
            match b:
                case Emp():
                    return Node(a0, a1)
                case One():
                    return Node(a0, a1)
                case Node(b0, b1):
                    return node(diff(a0, b0), diff(a1, b1))


def size_of(t):
    match t:
        case Emp():
            return 0
        case One():
            return 1
        case Node(l, r):
            return size_of(l) + size_of(r)


# the out-neighbours of node v as a set
def nbrs(v, lg):
    t = Emp()
    for j in range(8):
        t = insert(t, nbr(v, j, lg), lg)
    return t


# the out-neighbours of the members of f (a subtree at prefix p, d bits
# left): both subtrees fork, their neighbour sets merge
def expand(f, d, p, lg):
    match f:
        case Emp():
            return Emp()
        case One():
            return nbrs(p, lg)
        case Node(l, r):
            a = expand(l, d - 1, p * 2, lg)
            b = expand(r, d - 1, p * 2 + 1, lg)
            return union(a, b)


# one level per step until the frontier is empty
def levels(front, seen, lvl, reached, acc, lg):
    n = size_of(front)
    if n == 0:
        return (reached, acc)
    nxt = diff(expand(front, lg, 0, lg), seen)
    return levels(nxt, union(seen, nxt), lvl + 1, reached + n, acc + lvl * n, lg)


def size():
    return 20  # SIZE


def main():
    lg = size()
    start = insert(Emp(), 0, lg)
    return levels(start, insert(Emp(), 0, lg), 0, 0, 0, lg)
