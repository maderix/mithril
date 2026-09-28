# a tuple-returning int fork reached only through scalar-looking callers:
# it and its callers must stay splittable (dive form); the leaf is native
def leafwork(i, n):
    s = 0
    for j in range(n):
        s = (s * 31 + ((i + j) * 2654435761 & 4294967295)) & 4294967295
    return (s, n)


def batch(d, i):
    if d == 0:
        return leafwork(i, 50 + (i & 1))
    a = batch(d - 1, i * 2)
    b = batch(d - 1, i * 2 + 1)
    return ((a[0] + b[0]) & 4294967295, (a[1] + b[1]) & 4294967295)


def run(d):
    t = batch(d, 1)
    return (t[0] ^ t[1]) & 4294967295


def main():
    return run(12)
