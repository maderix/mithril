@data
class L:
    Nil: ()
    Cons: (h, t)


def fill(a, n):
    for i in range(n):
        a = array_set(a, i, (i * i + 7) & 1023)
    return a


def total(l, acc):
    match l:
        case Nil():
            return acc
        case Cons(h, t):
            return total(t, (acc * 31 + h) & 1048575)


# an array of boxed lists: buckets grow by consing onto the old bucket
def buckets(n, k):
    a = array_new(8, Nil())
    for j in range(n):
        b = (j * 5 + k) & 7
        a = array_set(a, b, Cons(j, array_get(a, b)))
    s = 0
    for b in range(8):
        s = (s * 7 + total(array_get(a, b), 0)) & 1048575
    return s


def work(k):
    a = fill(array_new(64, 0), 64)
    # b is used after c is made: the update must copy, b stays intact
    b = a
    c = array_set(a, 3, 99)
    s = 0
    for i in range(array_len(c)):
        s = (s + array_get(b, i) * 7 + array_get(c, i) + k) & 1048575
    return (s + buckets(40 + k, k)) & 1048575


def batch(d, k):
    if d == 0:
        return work(k)
    return (batch(d - 1, k * 2) + batch(d - 1, k * 2 + 1)) & 1048575


def main():
    return batch(6, 1)
