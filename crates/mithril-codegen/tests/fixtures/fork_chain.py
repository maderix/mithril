# A merge whose two branches are each a dependent chain of calls
# (bitonic's flow: `Node(merge(bump(a)), merge(bump(b)))`). After the first
# call suspends, the call that needs only its result must not wait for the
# whole independent branch to finish: the continuation splits three ways
# (the dependent chain, the independent branch, the join of both), so
# the net's own parallelism (both branches at once) survives the lowering.
# Checksum-only; the oracle is eval_core.


@data
class Tree:
    Leaf: (v,)
    Node: (l, r)


def build(d, x):
    if d == 0:
        return Leaf(x & 4294967295)
    return Node(build(d - 1, (x * 2) & 4294967295), build(d - 1, (x * 2 + 1) & 4294967295))


# a whole-tree pass (bitonic's warp_node)
def bump(t, k):
    match t:
        case Leaf(v):
            return Leaf(((v * 2654435761) + k) & 4294967295)
        case Node(a, b):
            return Node(bump(a, k), bump(b, k))


# the flow shape: each branch calls merge on the result of bump
def merge(d, k, t):
    match t:
        case Leaf(v):
            return Leaf(v)
        case Node(a, b):
            if d == 0:
                return Node(a, b)
            return Node(merge(d - 1, k, bump(a, k)), merge(d - 1, k, bump(b, k)))


def total(t):
    match t:
        case Leaf(v):
            return v
        case Node(a, b):
            return (total(a) + total(b)) & 4294967295


def main():
    # the depth is opaque to the net (an array length), so the program is
    # not folded at compile time
    d = array_len(array_new(8, 0))
    return total(merge(d - 2, 7, build(d, 1)))
