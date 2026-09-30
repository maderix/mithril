# Probe (review axis 2): a float shared by keep(x, n) and a closure.
# Correct: (4.75, 3.75)  -- by hand: n = 3, x = 2.5 * 1.5 = 3.75, y = keep(x, 3) = 3.75,
# r = (lambda z: z + x)(1.0) = 4.75; the reference interpreter (mithril oracle) agrees.

def ap(f, v, n):
    if n == 0:
        return f(v)
    return ap(f, v, n - 1)


def keep(x, n):
    if n == 0:
        return x
    return keep(x, n - 1)


def main():
    n = array_len(array_new(3, 0))
    x = array_get(array_new(n, 2.5), 0) * 1.5
    y = keep(x, n)
    r = ap(lambda z: z + x, 1.0, n)
    return (r, y)
