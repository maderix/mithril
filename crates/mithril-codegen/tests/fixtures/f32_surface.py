# The f32 surface: float literals, + - * /, unary minus, comparisons (NaN
# and signed zero), sqrt, f32() and int() conversions, with the type found
# through calls, returns, tuples, destructuring, constructors and arrays.
# A batch of 2^6 ray-sphere tests forks as a tree; a closure compares f32.
@data
class Hit:
    Miss: ()
    At: (t, id)


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def sphere(o, d, c, r, id):
    oc = sub(o, c)
    b = dot(oc, d)
    q = dot(oc, oc) - r * r
    disc = b * b - q
    if disc < 0.0:
        return Miss()
    t = -b - sqrt(disc)
    if t > 0.001:
        return At(t, id)
    return Miss()


def nearest(h, k):
    match h:
        case Miss():
            return k
        case At(t, id):
            match k:
                case Miss():
                    return h
                case At(u, j):
                    if u < t:
                        return k
                    return h


def ray(i):
    x = f32(i % 8) / 4.0 - 1.0
    y = f32(i // 8) / 4.0 - 1.0
    n = sqrt(x * x + y * y + 1.0)
    d = (x / n, y / n, 1.0 / n)
    o = (0.0, 0.0, -3.0)
    h = nearest(sphere(o, d, (0.0, 0.0, 0.0), 1.0, 1), sphere(o, d, (0.5, 0.25, 1.5), 0.75, 2))
    match h:
        case Miss():
            return 0
        case At(t, id):
            return int(t * 1000.0) * 3 + id


def batch(lo, n):
    if n == 1:
        return ray(lo)
    h = n // 2
    return (batch(lo, h) + batch(lo + h, n - h)) & 4294967295


def ieee():
    nan = sqrt(-1.0)
    z = 0.0
    nz = -z
    one = 1.0
    r = 0
    if nan < one or nan <= one or nan > one or nan >= one or nan == nan:
        r = r + 1
    if nan != nan:
        r = r + 2
    if z == nz and z <= nz and z >= nz:
        r = r + 4
    if one / nz < 0.0 and one / z > 0.0:
        r = r + 8
    return r


def convert():
    a = int(-2.75) + 10
    b = int(f32(-7) * 0.5) + 10
    c = int(sqrt(-1.0))
    d = int(f32(16777217))
    return (a, b, c, d)


def arrays(n):
    a = array_new(n, 0.0)
    for i in range(n):
        a = array_set(a, i, f32(i) * 0.25)
    s = 0.0
    for i in range(n):
        s = s + array_get(a, i)
    return int(s * 4.0)


# f32 inside a closure: its body runs on the net's rules at runtime
def thresholds(n):
    k = sqrt(2.0)
    g = lambda v: 1 if k <= f32(v) * 0.5 else 0
    s = 0
    for i in range(n):
        s = s + g(i)
    return s


def main():
    lo, hi = (0, 64)
    return (batch(lo, hi), ieee(), convert(), arrays(10), thresholds(array_len(array_new(10, 0))))
