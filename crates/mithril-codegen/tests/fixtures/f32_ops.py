# binary32 primitives on bit patterns: random operands, subnormals, huge
# values, conversions at their boundaries, and an iterated map
def rnd(x):
    return (x * 1103515245 + 12345) & 4294967295


# a finite, non-NaN pattern from a random word (exponent kept below 255)
def fin(x):
    e = (x >> 23) & 255
    if e == 255:
        return x & 4286578687
    return x


def mix(h, v):
    return ((h * 31) & 4294967295) ^ v


def ops(n, seed):
    x = seed
    h = 0
    for i in range(n):
        x = rnd(x)
        a = fin(x)
        x = rnd(x)
        b = fin(x)
        h = mix(h, f32_add(a, b))
        h = mix(h, f32_sub(a, b))
        h = mix(h, f32_mul(a, b))
        if (b & 2147483647) != 0:
            h = mix(h, f32_div(a, b))
        h = mix(h, f32_sqrt(a & 2147483647))
        h = mix(h, f32_lt(a, b))
        h = mix(h, f32_from_u32(x))
        h = mix(h, f32_to_u32(a))
    return h


# edge values: subnormal, smallest normal, max finite, 2^32 - ulp, 2^32,
# -0, 0.5, 1.5, 4294967040.0
def edges():
    vs = array_new(9, 0)
    vs = array_set(vs, 0, 1)
    vs = array_set(vs, 1, 8388608)
    vs = array_set(vs, 2, 2139095039)
    vs = array_set(vs, 3, 1333788671)
    vs = array_set(vs, 4, 1333788672)
    vs = array_set(vs, 5, 2147483648)
    vs = array_set(vs, 6, 1056964608)
    vs = array_set(vs, 7, 1069547520)
    vs = array_set(vs, 8, 1333788670)
    h = 0
    for i in range(9):
        a = array_get(vs, i)
        h = mix(h, f32_to_u32(a))
        for j in range(9):
            b = array_get(vs, j)
            h = mix(h, f32_add(a, b))
            h = mix(h, f32_mul(a, b))
            h = mix(h, f32_lt(a, b))
    return h


# an iterated chaotic map in f32: x <- 3.9 x (1 - x)
def logistic(n):
    one = 1065353216
    r = 1081711002
    x = 1056964608
    for i in range(n):
        x = f32_mul(f32_mul(r, x), f32_sub(one, x))
    return x


def main():
    return (ops(20000, 7) + edges() + logistic(100000) + f32_from_u32(4294967295)) & 4294967295
