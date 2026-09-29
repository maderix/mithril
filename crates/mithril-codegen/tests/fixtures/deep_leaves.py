# A fork tree wide enough to fill every device lane, whose leaves are deep
# boxed chains: in the WORK phase a lane runs a leaf's chain in the
# sequential world, where the dive budget must bound the native recursion
# depth (the device stack is 32 KiB per thread).
@data
class Box:
    B: (v,)


def chain(n, x):
    if n == 0:
        return B(x)
    match chain(n - 1, x):
        case B(v):
            return B((v * 3 + 1) & 4294967295)


def leaves(d, x, n):
    if d == 0:
        match chain(n, x):
            case B(v):
                return v
    a = leaves(d - 1, (x * 2) & 4294967295, n)
    b = leaves(d - 1, (x * 2 + 1) & 4294967295, n)
    return (a + b) & 4294967295


def main():
    n = array_len(array_new(100, 0))
    return leaves(16, 1, n)
