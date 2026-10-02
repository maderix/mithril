# A tree walk whose second self call needs the first one's result: a chain,
# not a fork, so it keeps its native form.
@data
class T:
    Leaf: (v,)
    Node: (l, r)


def build(d, x):
    if d == 0:
        return Leaf(x & 1023)
    return Node(build(d - 1, x * 2), build(d - 1, x * 2 + 1))


def walk(t, acc):
    match t:
        case Leaf(v):
            return (acc * 31 + v) & 4294967295
        case Node(l, r):
            return walk(r, walk(l, acc))


def main():
    t = build(12, array_len(array_new(1, 0)))
    return (walk(t, 1) + walk(t, 2)) & 4294967295
