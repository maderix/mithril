# Spike 5's W2 at runtime: a closure over work that does not depend on
# its parameter, built once, applied N times. N is the loop bound; the
# closure's free work runs once (a compiled call whose result the net
# shares), each application costs a constant.
def heavy(k):
    if k < 2:
        return k
    return heavy(k - 1) + heavy(k - 2)


def mk(k):
    return lambda x: x + heavy(k)


def loop(g, i, acc):
    if i == 0:
        return acc
    return loop(g, i - 1, acc + g(i))


def main():
    k = array_len(array_new(20, 0))
    return loop(mk(k), 4096, 0)
