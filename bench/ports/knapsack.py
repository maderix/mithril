# knapsack: 0/1 knapsack by dynamic programming over a table of best values
# per capacity. Items are added one at a time, so rows run in order; inside
# a row each capacity reads only the previous row, so the capacities of one
# row are independent. Weights and values are xorshift hashes of the item
# index. Checksum: best value at full capacity, u32. Sizes: small items 20,
# capacity 1000; big items 8000, capacity 100000.


def prng(x):
    b = x ^ ((x << 13) & 4294967295)
    d = b ^ (b >> 17)
    return d ^ ((d << 5) & 4294967295)


def weight(i):
    return (prng((i + 1) * 2654435761 & 4294967295) & 2047) + 1


def value(i):
    return (prng((i + 1) * 40503 & 4294967295) & 4095) + 1


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
        row = add_item(row, cap, weight(i), value(i))
    return array_get(row, cap) & 4294967295


def main():
    return solve(8000, 100000)
