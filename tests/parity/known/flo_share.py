# Probe (review axis 2): floats shared between compiled code and the net.
# Correct: (4.75, 9.5, 4.0)  -- the reference interpreter (mithril oracle).

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
    r = ap(lambda z: z + x, 1.0, n)
    s = ap(lambda z: z * 2.0, r, n)
    y = keep(x, n)
    return (r, s, y + 0.25)
