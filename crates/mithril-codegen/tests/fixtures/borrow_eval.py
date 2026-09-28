@data
class E:
    X: ()
    K: (v,)
    Add: (a, b)
    Mul: (a, b)


def gen(d, h):
    if d == 0:
        if (h & 1) == 0:
            return X()
        return K(h & 255)
    a = gen(d - 1, (h * 3 + 1) & 65535)
    b = gen(d - 1, (h * 5 + 2) & 65535)
    if (h & 2) == 0:
        return Add(a, b)
    return Mul(a, b)


# reads the shared tree: lent, never owned
def ev(e, x):
    match e:
        case X():
            return x
        case K(v):
            return v
        case Add(a, b):
            return (ev(a, x) + ev(b, x)) & 1048575
        case Mul(a, b):
            return (ev(a, x) * ev(b, x)) & 1048575


def size(e):
    match e:
        case X():
            return 1
        case K(v):
            return 1
        case Add(a, b):
            return 1 + size(a) + size(b)
        case Mul(a, b):
            return 1 + size(a) + size(b)


# the same tree evaluated at j points: shared across calls
def fit(j, e, acc):
    if j == 0:
        return (acc + size(e)) & 1048575
    return fit(j - 1, e, (acc + ev(e, j)) & 1048575)


def batch(d, s):
    if d == 0:
        return fit(40, gen(6, s), 0)
    a = batch(d - 1, (s * 3 + 1) & 65535)
    b = batch(d - 1, (s * 5 + 2) & 65535)
    return (a + b) & 1048575


def main():
    return batch(8, 7)
