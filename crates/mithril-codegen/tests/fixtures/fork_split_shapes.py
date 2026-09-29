# The frame split's shapes: after `x = bump(a)` suspends, the rest splits
# into P (independent), D (needs x only) and J2 (the join); each function
# below is one shape, including the three that fall back to the two-way
# split. `t` is a boxed value shared by D, P and J2 (captured by each
# record, counted once per use). Checksum-only; the oracle is eval_core.
@data
class Tree:
    Leaf: (v,)
    Node: (l, r)


def build(d, x):
    if d == 0:
        return Leaf(x & 4294967295)
    return Node(build(d - 1, (x * 2) & 4294967295), build(d - 1, (x * 2 + 1) & 4294967295))


def bump(t, k):
    match t:
        case Leaf(v):
            return Leaf(((v * 2654435761) + k) & 4294967295)
        case Node(a, b):
            return Node(bump(a, k), bump(b, k))


def total(t):
    match t:
        case Leaf(v):
            return v
        case Node(a, b):
            return (total(a) + total(b)) & 4294967295


# D = {tt = total(t); y = bump(x, tt)}, P = {z = bump(b, total(t))}, J2 reads t
def shared_env(t, a, b):
    x = bump(a, 1)
    tt = total(t) & 7
    y = bump(x, tt)
    z = bump(b, total(t) & 3)
    return Node(Node(y, z), bump(t, 7))


# J2 reads x: no D (two-way split)
def join_reads_x(t, a, b):
    x = bump(a, 1)
    y = bump(x, 2)
    z = bump(b, 3)
    return Node(Node(y, z), x)


# D would have two live-outs: no D
def two_live_outs(t, a, b):
    x = bump(a, 1)
    y1 = bump(x, 2)
    y2 = bump(x, 9)
    z = bump(b, 3)
    return Node(Node(y1, y2), z)


# D has no call: no D
def pure_dependent(t, a, b):
    x = bump(a, 1)
    y = Node(x, Leaf(1))
    z = bump(b, 3)
    return Node(y, z)


def main():
    d = array_len(array_new(7, 0))
    t = build(d - 3, 5)
    a = build(d, 1)
    b = build(d, 2)
    s1 = total(shared_env(t, a, b))
    s2 = total(join_reads_x(t, a, b))
    s3 = total(two_live_outs(t, a, b))
    s4 = total(pure_dependent(t, a, b))
    return (s1 + s2 * 3 + s3 * 5 + s4 * 7) & 4294967295
