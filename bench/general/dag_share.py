# A heavily shared DAG: each level reuses its child twice (Node(x, x)), so
# a depth-d structure has d distinct nodes but 2^d paths. The program
# walks it several ways (sum, weighted fold, rebuild with a map that
# keeps sharing), keeping the old versions alive. Non-linear throughout.
@data
class T:
    Leaf: (v,)
    Node: (l, r)


def mk(d, v):
    if d == 0:
        return Leaf(v)
    x = mk(d - 1, v * 3 + d)
    return Node(x, x)


def total(t):
    match t:
        case Leaf(v):
            return v
        case Node(l, r):
            return (total(l) + total(r)) & 4294967295


def depth_mix(t, k):
    match t:
        case Leaf(v):
            return (v * k) & 65535
        case Node(l, r):
            return (depth_mix(l, k + 1) * 31 + depth_mix(r, k + 2)) & 4294967295


def bump(t, c):
    match t:
        case Leaf(v):
            return Leaf((v + c) & 1023)
        case Node(l, r):
            x = bump(l, c)
            return Node(x, x)


def run(n):
    s = 0
    for i in range(n):
        a = mk(16, i)
        b = bump(a, i)
        s = (s + total(a) + depth_mix(b, i & 7) + total(b)) & 4294967295
    return s


def main():
    return run(4000)
