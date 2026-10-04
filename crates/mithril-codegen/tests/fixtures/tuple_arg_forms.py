# A tuple argument that is itself a branch, a let or a call: the running best
# of a search passed back into the loop as closer(best, candidate) (a call),
# as a choice between two tuples (a branch) and through a binding (a let).
def cand(i):
    return (i * 7 % 13, i, i * 3)


def closer(h, k):
    if k[0] < h[0]:
        return k
    return h


def best_call(i, n, b):
    if i == n:
        return b
    return best_call(i + 1, n, closer(b, cand(i)))


def best_branch(i, n, b):
    if i == n:
        return b
    k = cand(i)
    return best_branch(i + 1, n, k if k[0] < b[0] else b)


def best_let(i, n, b):
    if i == n:
        return b
    k = cand(i)
    return best_let(i + 1, n, (k[0] + b[0], k[1], b[2]))


def main():
    n = array_len(array_new(40, 0))
    a = best_call(0, n, (100, 0, 0))
    b = best_branch(0, n, (100, 0, 0))
    c = best_let(0, n, (0, 0, 7))
    return a[0] + a[1] * 1000 + a[2] + b[0] * 7 + b[1] * 11 + c[0] * 13 + c[1] + c[2]
