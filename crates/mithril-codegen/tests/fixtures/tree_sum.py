@data
class Tree:
    Leaf: (v,)
    Node: (l, r)

def mk(d, v):
    if d == 0:
        return Leaf(v)
    return Node(mk(d - 1, v * 2), mk(d - 1, v * 2 + 1))

def sumtree(t):
    match t:
        case Leaf(v):
            return v
        case Node(l, r):
            return sumtree(l) + sumtree(r)

def main():
    return sumtree(mk(12, 1))
