# Self calls in different arms of a branch are alternatives, not a fork:
# `walk` recurses once per call whichever arm runs (linear recursion, the
# shape of a path tracer's bounce), so it has a native form, and so does
# its caller `via_walk`. `tree` and `choose` make two self calls in one
# arm, `cond_fork` one in a condition and one in an arm, `seq_fork` one
# before a branch and one in an arm: each can make two, and stays
# splittable.
def walk(n, k):
    if n == 0:
        return (k, 0)
    if n % 3 == 0:
        r = walk(n - 1, k + 1)
        return (r[0] * 2, r[1] + 1)
    r = walk(n - 1, k)
    return (r[0] + 1, r[1])


def tree(n):
    if n < 2:
        return n
    return tree(n - 1) + tree(n - 2)


def choose(n):
    if n < 2:
        return 1
    if n % 2 == 0:
        return choose(n - 1) + choose(n - 2)
    return choose(n - 1) * 3


def cond_fork(n):
    if n < 2:
        return 1
    if cond_fork(n - 1) > 3:
        return cond_fork(n - 2) + 1
    return 2


def seq_fork(n):
    if n < 2:
        return 1
    a = seq_fork(n - 1)
    if a % 2 == 0:
        return a + seq_fork(n - 2)
    return a + 1


def via_walk(n):
    r = walk(n, 2)
    return r[0] + r[1]


def main():
    n = array_len(array_new(20, 0))
    return (walk(n, 1), tree(n), choose(n), cond_fork(n), seq_fork(n), via_walk(n))
