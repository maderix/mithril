# fsum: the f32 sum of 2^S values, value i = (h_i & 2^24 - 1) / 2^24 - 0.5
# with h_i a xorshift hash of (i + 1) * 2654435761 (exact in f32, in
# [-0.5, 0.5)). The sum is a fixed-shape tree: halves of the index range
# summed recursively (both fork) and added. The shape depends only on S, so
# the rounding, and the result's bits, are the same for every thread count
# and schedule. main returns the f32 sum, printed as its bit pattern.
# The C twin's OpenMP reduction and the Rust twin's rayon sum group the
# additions by thread and by work split, so their bits may differ from this
# and from run to run. The same fixed-shape tree written in C or Rust
# (main_tree.c, rust/src/bin/fsum_tree.rs) prints the same bits as this.
# Sizes (S): small 18 expect 3231587146 (-4.940831); big 24 expect
# 3232996432 (-5.612831).


def prng(x):
    b = x ^ ((x << 13) & 4294967295)
    d = b ^ (b >> 17)
    return d ^ ((d << 5) & 4294967295)


def val(i):
    h = prng(((i + 1) * 2654435761) & 4294967295)
    return f32(h & 16777215) / 16777216.0 - 0.5


def tsum(lo, k):
    if k == 0:
        return val(lo)
    return tsum(lo, k - 1) + tsum(lo + (1 << (k - 1)), k - 1)


def size():
    return 24  # SIZE


def main():
    return tsum(0, size())
