@data
class L:
    Nil: ()
    Cons: (h, t)


def is_open(g, n):
    return 1 if array_get(g, n) != 0 else 0


# a leaf helper (after inlining is_open): reads g (borrowed), writes q and
# d (owned), returns both arrays and an int in one tuple
def step(n, v, g, q, d, tail):
    if is_open(g, n) == 1:
        if array_get(d, n) == 0:
            return (array_set(q, tail, n), array_set(d, n, v), tail + 1)
    return (q, d, tail)


# an array-returning loop (owned param threaded through the loop)
def fill(a, n, s):
    for i in range(n):
        a = array_set(a, i, ((i * 2654435761 + s) >> 7) & 3)
    return a


# a read-only loop over a borrowed array, with an if/else accumulator
# (if-converted into selects)
def score(a, n):
    acc = 0
    cnt = 0
    for i in range(n):
        w = array_get(a, i)
        if w != 0:
            acc = ((acc * 31) & 1048575) ^ (w * (i + 1))
            cnt = cnt + 1
        else:
            acc = (acc * 31) & 1048575
    return (acc + cnt * 7) & 1048575


# a queue walk: the tuple results of step feed the next iteration
def walk(g, n):
    q = array_new(n, 0)
    d = array_new(n, 0)
    q = array_set(q, 0, 0)
    d = array_set(d, 0, 1)
    head = 0
    tail = 1
    while head < tail:
        c = array_get(q, head)
        head = head + 1
        v = array_get(d, c) + 1
        r = step((c + 1) % n, v, g, q, d, tail)
        q = r[0]
        d = r[1]
        tail = r[2]
        r = step((c + 5) % n, v, g, q, d, tail)
        q = r[0]
        d = r[1]
        tail = r[2]
    return score(d, n)


# the callee lends y while x moves in: with x and y the same array the
# write must not show through y
def upd2(x, y, i):
    x2 = array_set(x, i, 5)
    return (array_get(y, i) * 3 + array_get(x2, i)) & 1048575


def alias(a):
    return upd2(a, a, 1)


# b = a shares the array: the update must copy and b stays intact
def shared(a):
    b = a
    c = array_set(a, 2, 77)
    return (array_get(b, 2) * 5 + array_get(c, 2)) & 1048575


# array-valued if in value position: both arms consume a
def pick(a, c):
    b = array_set(a, 0, 9) if c > 0 else array_set(a, 1, 8)
    return (array_get(b, 0) * 3 + array_get(b, 1)) & 1048575


# a tuple-returning function that is not native (it builds a list)
def split(l, acc, n):
    match l:
        case Nil():
            return (acc, n)
        case Cons(h, t):
            return split(t, Cons(h * 2, acc), n + h)


def total(l, s):
    match l:
        case Nil():
            return s
        case Cons(h, t):
            return total(t, (s * 13 + h) & 1048575)


def build(k, acc):
    if k == 0:
        return acc
    return build(k - 1, Cons(k, acc))


def listy(k):
    r = split(build(k, Nil()), Nil(), 0)
    l = r[0]
    n = r[1]
    return (total(l, 0) + n) & 1048575


def work(k):
    g = fill(array_new(64, 0), 64, k)
    s = walk(g, 64)
    # the same array handed to two native calls: the second must see the
    # original (the first copies before writing in place)
    t = (pick(g, k & 1) + pick(g, 1 - (k & 1))) & 1048575
    base = fill(array_new(8, 1), 8, k)
    u = (alias(base) + shared(base) + score(base, 8)) & 1048575
    return (s * 3 + t * 5 + u + listy(k & 15) + array_len(g)) & 1048575


def batch(d, k):
    if d == 0:
        return work(k)
    return (batch(d - 1, k * 2) + batch(d - 1, k * 2 + 1)) & 1048575


def main():
    return batch(5, 3)
