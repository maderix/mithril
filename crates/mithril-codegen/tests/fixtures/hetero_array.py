@data
class L:
    Nil: ()
    Cons: (h, t)


# an array that starts all-int and later stores a list: stored raw, then
# converted to tagged elements on the first non-int write
def hetero(k):
    a = array_new(4, k)
    a = array_set(a, 1, k * 3 - 7)
    s = array_get(a, 0) + array_get(a, 1)
    b = array_set(a, 2, Cons(k, Nil()))
    match array_get(b, 2):
        case Cons(h, t):
            s = s + h * 11
        case Nil():
            s = s - 1
    return s + array_get(b, 1) + array_get(a, 1)


def main():
    s = 0
    for k in range(20):
        s = s + hetero(k - 9)
    return s
