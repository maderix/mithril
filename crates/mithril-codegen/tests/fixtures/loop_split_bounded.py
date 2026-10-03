# Loops whose per-iteration work is bounded (straight-line helpers, no loop
# and no recursion) stay one native loop: a split leaf per iteration would
# cost more than the iteration. Results must equal the sequential ones.
def weight(i):
    return (i * 2654435761 + 7) & 2047


def best(row, c, w, v):
    keep = array_get(row, c)
    if c < w:
        return keep
    take = array_get(row, c - w) + v
    if take > keep:
        return take
    return keep


def add_item(row, cap, w, v):
    nr = array_new(cap + 1, 0)
    for c in range(cap + 1):
        nr = array_set(nr, c, best(row, c, w, v))
    return nr


def solve(items, cap):
    row = array_new(cap + 1, 0)
    for i in range(items):
        row = add_item(row, cap, weight(i) + 1, (weight(i + 99) & 1023) + 1)
    return array_get(row, cap)


def mix(n):
    s = 0
    for i in range(n):
        s = (s * 31 + weight(i)) & 4294967295
    return s


def main():
    return solve(30, 500) * 7 + mix(200)
