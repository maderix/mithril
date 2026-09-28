# Spike 5's W2 shape as a program: a closure over work that does not
# depend on its parameter (heavy(k), k only known at runtime), built once
# and applied n times. Written the natural way, without hoisting heavy(k)
# out of the lambda by hand: the net does the work once and shares it
# across every application; a strict evaluation runs it per call.
def heavy(k):
    if k < 2:
        return k
    return heavy(k - 1) + heavy(k - 2)


def mk(k):
    return lambda x: x + heavy(k)


def loop(g, i, acc):
    if i == 0:
        return acc
    return loop(g, i - 1, (acc + g(i)) & 1048575)


def run(n):
    k = array_len(array_new(22, 0))
    return loop(mk(k), n, 0)


def main():
    return run(20000)
