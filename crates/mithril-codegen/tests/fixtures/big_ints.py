# Ints are 64-bit and wrap at 2^64; a value past 56 bits crosses a port as
# a boxed int. Each lowering moves such values: native arithmetic, a dive
# recursion that suspends holding boxed arguments, raw int arrays,
# constructor fields, tuples, a fold join, a branch, sharing and readback.

@data
class Tree:
    Leaf: (v,)
    Node: (l, r)


# native: passes 2^56 and wraps at 2^64
def mix(x):
    return x * 6364136223846793005 + 1442695040888963407


# a dive chain whose argument is boxed at every suspension
def chain(n, x):
    if n == 0:
        return x
    return chain(n - 1, mix(x) ^ (x >> 7))


# boxed leaves: the field cannot be carried in the port
def mk(d, v):
    if d == 0:
        return Leaf(v)
    return Node(mk(d - 1, v * 3 + 1000000000000000001), mk(d - 1, v * 5 - 777777777777777777))


def sumtree(t):
    match t:
        case Leaf(v):
            return v
        case Node(l, r):
            return sumtree(l) + sumtree(r)


# a proven fold whose partial sums pass 2^56: the join adds boxed ints
def total(n):
    s = 0
    for i in range(n):
        s = s + i * 1099511627776
    return s


# a raw int array of values past 56 bits
def fill(n, base):
    a = array_new(n, base)
    for i in range(n):
        a = array_set(a, i, array_get(a, i) + i * (1 << 50))
    return a


def scan(a):
    m = 0
    for i in range(array_len(a)):
        m = m * 31 + array_get(a, i)
    return m


def pair(x):
    return (x, x * 3)


def pick(x):
    if x > (1 << 58):
        return x - 1
    return x + 1


def main():
    c = chain(3000, 12345)
    t = sumtree(mk(10, 1 << 55))
    s = total(300000)
    a = fill(1000, 1 << 59)
    p = pair(c)
    shared = c + c
    edge = 9223372036854775807 + (c & 1) + 1
    return (c, t, s, scan(a), array_get(a, 999), p, pick(c), pick(t & 7), shared, edge)
