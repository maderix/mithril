# Arrays with live old versions: a snapshot is taken every few steps and
# read later, so writes after a snapshot must copy (copy-on-write) while
# unshared writes stay in place.
def run(n):
    a = array_new(256, 1)
    snap = a
    s = 0
    for i in range(n):
        j = (i * 37) & 255
        a = array_set(a, j, (array_get(a, (j + 1) & 255) + i) & 65535)
        if (i & 63) == 0:
            snap = a
        s = (s + array_get(snap, j) + array_get(a, (j * 7) & 255)) & 4294967295
    return s


def main():
    return run(3000000)
