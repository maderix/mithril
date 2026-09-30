# hashbuild: insert 2^S keys into a table of 2^S slots, with no shared
# table. Key i is a (S + 4)-bit xorshift hash of i (so some keys repeat and
# an insert of a present key changes nothing); its slot is the top S bits
# of key * 2654435761. The table is an immutable binary trie over the slot
# bits (MSB at the root), a slot holding the chain of its distinct keys.
# It is built as a fold over the index range: a leaf is the one-key table,
# and two tables merge by walking both tries together (both subtrees fork)
# and taking the union of the chains where both hold a slot.
# Checksum: (occupied slots, sum over occupied slots s of mix(s, summary)),
# u32, where a slot's summary is (sum of its keys + count * 2654435761),
# independent of the order its keys arrived in.
# Sizes (S): small 12 expect (2581, 2337151009); big 20 expect
# (660939, 2427044196).


@data
class Chain:
    Nil: ()
    Cons: (k, t)


@data
class Table:
    Free: ()
    Slot: (c,)
    Node: (l, r)


def prng(x):
    b = x ^ ((x << 13) & 4294967295)
    d = b ^ (b >> 17)
    return d ^ ((d << 5) & 4294967295)


def mix(i, v):
    return prng((((i + 1) * 2654435761) & 4294967295) ^ ((v * 2246822519) & 4294967295))


def key(i, lg):
    return prng((((i + 1) * 2654435761) & 4294967295) ^ 1779033703) & ((1 << (lg + 4)) - 1)


def slot(k, lg):
    return ((k * 2654435761) & 4294967295) >> (32 - lg)


# the one-key table: the slot's bits spelled from the deepest level up
def spell(d, s, r):
    if d == 0:
        return r
    if (s & 1) == 0:
        return spell(d - 1, s >> 1, Node(r, Free()))
    return spell(d - 1, s >> 1, Node(Free(), r))


def has(c, k):
    match c:
        case Nil():
            return 0
        case Cons(h, t):
            if h == k:
                return 1
            return has(t, k)


# the keys of a not already in b, consed onto b
def union(a, b):
    match a:
        case Nil():
            return b
        case Cons(k, t):
            if has(b, k) == 1:
                return union(t, b)
            return union(t, Cons(k, b))


def merge(a, b):
    match a:
        case Free():
            return b
        case Slot(ca):
            match b:
                case Free():
                    return Slot(ca)
                case Slot(cb):
                    return Slot(union(ca, cb))
                case Node(b0, b1):
                    return Node(b0, b1)
        case Node(a0, a1):
            match b:
                case Free():
                    return Node(a0, a1)
                case Slot(cb):
                    return Node(a0, a1)
                case Node(b0, b1):
                    return Node(merge(a0, b0), merge(a1, b1))


# the fold over 2^k keys from lo: both halves fork, then merge
def build(lo, k, lg):
    if k == 0:
        x = key(lo, lg)
        return spell(lg, slot(x, lg), Slot(Cons(x, Nil())))
    a = build(lo, k - 1, lg)
    b = build(lo + (1 << (k - 1)), k - 1, lg)
    return merge(a, b)


def summary(c, n, s):
    match c:
        case Nil():
            return (s + n * 2654435761) & 4294967295
        case Cons(k, t):
            return summary(t, n + 1, s + k)


# (occupied, hash) of the subtree whose slots start at prefix p
def digest(t, p):
    match t:
        case Free():
            return (0, 0)
        case Slot(c):
            return (1, mix(p, summary(c, 0, 0)))
        case Node(l, r):
            a = digest(l, p * 2)
            b = digest(r, p * 2 + 1)
            return (a[0] + b[0], (a[1] + b[1]) & 4294967295)


def size():
    return 20  # SIZE


def main():
    lg = size()
    return digest(build(0, lg, lg), 0)
