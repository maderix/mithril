# An array of tuples built by the net (inside a closure applied at
# runtime) and read by compiled code. Open bug (design.md section 12):
# the element's fields are read with the wrong representation.
def ap(f, s, n):
    if n == 0:
        return f(s)
    return ap(f, s, n - 1)


def main():
    n = array_len(array_new(3, 0))
    a = ap(lambda y: array_new(2, (y * 3, 1)), n, n)
    return array_get(a, 1)[0] + 1
