# A large read-only tree shared by 2^14 parallel forks (the k-d tree
# port at a mid size): every fork takes a reference to the root, so the
# root's count climbs past 255 under 16 threads. An 8-bit saturating
# count wrapped through 0 for a moment and a reader freed the shared tree
# (a runtime panic at 16 threads, never at 1).
@data
class Pts:
    Nil: ()
    Cons: (x, y, i, rest)


@data
class Tree:
    Empty: ()
    Leaf: (x, y, i)
    Bucket: (ps,)
    Node: (axis, mid, lo, hi)


def prng(x):
    b = x ^ ((x << 13) & 4294967295)
    d = b ^ (b >> 17)
    return d ^ ((d << 5) & 4294967295)


def px(i):
    return prng((i + 1) * 2654435761 & 4294967295) & 65535


def py(i):
    return prng((i + 1) * 40503 & 4294967295) & 65535


def points(lo, hi, acc):
    if lo == hi:
        return acc
    return points(lo, hi - 1, Cons(px(hi - 1), py(hi - 1), hi - 1, acc))


# partition by coordinate on `axis` against `mid`: (below, at-or-above)
def part(ps, axis, mid, below, above):
    match ps:
        case Nil():
            return (below, above)
        case Cons(x, y, i, rest):
            c = x if axis == 0 else y
            if c < mid:
                return part(rest, axis, mid, Cons(x, y, i, below), above)
            return part(rest, axis, mid, below, Cons(x, y, i, above))


# the cell is [x0, x0+wx) x [y0, y0+wy); the split axis alternates, an
# axis of width 1 cannot split, and a cell that cannot split at all holds
# its points in a bucket (coincident points)
def build(ps, axis, x0, y0, wx, wy):
    match ps:
        case Nil():
            return Empty()
        case Cons(x, y, i, rest):
            match rest:
                case Nil():
                    return Leaf(x, y, i)
                case Cons(a, b, c, d):
                    if wx == 1 and wy == 1:
                        return Bucket(ps)
                    ax = axis
                    if (axis == 0 and wx == 1) or (axis == 1 and wy == 1):
                        ax = 1 - axis
                    if ax == 0:
                        h = wx >> 1
                        r = part(ps, 0, x0 + h, Nil(), Nil())
                        return Node(0, x0 + h, build(r[0], 1, x0, y0, h, wy), build(r[1], 1, x0 + h, y0, wx - h, wy))
                    h = wy >> 1
                    r = part(ps, 1, y0 + h, Nil(), Nil())
                    return Node(1, y0 + h, build(r[0], 0, x0, y0, wx, h), build(r[1], 0, x0, y0 + h, wx, wy - h))


def d2(ax, ay, bx, by):
    dx = ax - bx
    dy = ay - by
    return dx * dx + dy * dy


def scan(ps, qx, qy, bd, bi):
    match ps:
        case Nil():
            return (bd, bi)
        case Cons(x, y, i, rest):
            d = d2(qx, qy, x, y)
            if d < bd:
                return scan(rest, qx, qy, d, i)
            return scan(rest, qx, qy, bd, bi)


# nearest to (qx, qy): best = (distance, id) so far
def nearest(t, qx, qy, bd, bi):
    match t:
        case Empty():
            return (bd, bi)
        case Leaf(x, y, i):
            d = d2(qx, qy, x, y)
            if d < bd:
                return (d, i)
            return (bd, bi)
        case Bucket(ps):
            return scan(ps, qx, qy, bd, bi)
        case Node(axis, mid, lo, hi):
            q = qx if axis == 0 else qy
            s = q - mid
            if s < 0:
                r = nearest(lo, qx, qy, bd, bi)
                if s * s < r[0]:
                    return nearest(hi, qx, qy, r[0], r[1])
                return r
            r = nearest(hi, qx, qy, bd, bi)
            if s * s < r[0]:
                return nearest(lo, qx, qy, r[0], r[1])
            return r


def query(t, j):
    qx = prng((j + 7) * 2246822519 & 4294967295) & 65535
    qy = prng((j + 7) * 3266489917 & 4294967295) & 65535
    r = nearest(t, qx, qy, 4294967295, 0)
    return ((r[1] * 2654435761) + r[0]) & 4294967295


# the queries as a balanced fork tree over the shared tree
def qbatch(t, d, j):
    if d == 0:
        return query(t, j)
    a = qbatch(t, d - 1, j * 2)
    b = qbatch(t, d - 1, j * 2 + 1)
    return (a + b) & 4294967295


def main():
    n = 14
    t = build(points(0, 1 << n, Nil()), 0, 0, 0, 65536, 65536)
    return qbatch(t, 14, 0)
