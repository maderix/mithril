# Cornell box, path tracing.
#
# The scene of cornell_whitted.py (the same room, emitter, mirror sphere,
# glass sphere and block), rendered by Monte Carlo path tracing: every
# pixel averages spp() paths. A path meets up to six surfaces. At a
# diffuse surface it takes direct light from a random point on the
# emitter (next-event estimation) and continues in a cosine-weighted
# random direction; mirror and glass continue specularly (glass picks
# reflection or refraction with the Fresnel probability). Emission is
# counted where the camera sees the emitter, directly or through mirror
# and glass; after a diffuse bounce light comes by next-event estimation
# only, so none is counted twice (and caustics are left out). As in the
# Whitted demo, a shadow ray passes through glass dimmed instead of being
# blocked.
#
# Randomness is a hash of (pixel, sample, bounce): the image is the same
# on every run, thread count and device. No Russian roulette: every path
# has the same bounce limit.
#
# main() returns (width, height, pixels) like the Whitted demo:
# `mithril run --image out.ppm demos/cornell_path.py`.


def size():
    return 256


def spp():
    return 64


# ---- vectors: (x, y, z) tuples of f32 ----

def add(a, b):
    return (a[0] + b[0], a[1] + b[1], a[2] + b[2])


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def scale(a, s):
    return (a[0] * s, a[1] * s, a[2] * s)


def mul(a, b):
    return (a[0] * b[0], a[1] * b[1], a[2] * b[2])


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def unit(a):
    return scale(a, 1.0 / sqrt(dot(a, a)))


def reflect(d, n):
    return sub(d, scale(n, 2.0 * dot(d, n)))


def fmin(a, b):
    return a if a < b else b


def fmax(a, b):
    return a if a > b else b


# ---- randomness: a 32-bit integer hash, and a float in [0, 1) ----

def hash(x):
    x = ((x >> 16) ^ x) * 73244475 & 4294967295
    x = ((x >> 16) ^ x) * 73244475 & 4294967295
    return (x >> 16) ^ x


def unit_float(h):
    return f32(h >> 8) * (1.0 / 16777216.0)


# ---- the scene (as in cornell_whitted.py) ----

def far():
    return 1e30


def eps():
    return 1e-4


def miss():
    return (far(), (0.0, 0.0, 0.0), 0)


def closer(h, k):
    if k[0] < h[0]:
        return k
    return h


# materials: 1 white, 2 red, 3 green, 4 emitter, 5 mirror, 6 glass
def albedo(m):
    if m == 2:
        return (0.63, 0.065, 0.05)
    if m == 3:
        return (0.14, 0.45, 0.091)
    return (0.725, 0.71, 0.68)


def within(a, b):
    return a >= -1.0 and a <= 1.0 and b >= -1.0 and b <= 1.0


def wall_x(o, d, c, mat):
    if d[0] == 0.0:
        return miss()
    t = (c - o[0]) / d[0]
    p = add(o, scale(d, t))
    if t > eps() and within(p[1], p[2]):
        return (t, (-c, 0.0, 0.0), mat)
    return miss()


def wall_y(o, d, c):
    if d[1] == 0.0:
        return miss()
    t = (c - o[1]) / d[1]
    p = add(o, scale(d, t))
    if t > eps() and within(p[0], p[2]):
        lit = c > 0.0 and fmax(p[0] * p[0], p[2] * p[2]) < 0.0625
        return (t, (0.0, -c, 0.0), 4 if lit else 1)
    return miss()


def back_wall(o, d):
    if d[2] == 0.0:
        return miss()
    t = (1.0 - o[2]) / d[2]
    p = add(o, scale(d, t))
    if t > eps() and within(p[0], p[1]):
        return (t, (0.0, 0.0, -1.0), 1)
    return miss()


def sphere(o, d, c, r, mat):
    oc = sub(o, c)
    b = dot(oc, d)
    disc = b * b - (dot(oc, oc) - r * r)
    if disc < 0.0:
        return miss()
    s = sqrt(disc)
    t = -b - s
    if t < eps():
        t = -b + s
    if t < eps():
        return miss()
    return (t, scale(sub(add(o, scale(d, t)), c), 1.0 / r), mat)


def slab(o, d, lo, hi):
    t0 = (lo - o) / d
    t1 = (hi - o) / d
    return (fmin(t0, t1), fmax(t0, t1))


def block(o, d, lo, hi):
    x = slab(o[0], d[0], lo[0], hi[0])
    y = slab(o[1], d[1], lo[1], hi[1])
    z = slab(o[2], d[2], lo[2], hi[2])
    near = fmax(x[0], fmax(y[0], z[0]))
    fr = fmin(x[1], fmin(y[1], z[1]))
    if near > fr or near < eps():
        return miss()
    if near == x[0]:
        n = (-1.0 if d[0] > 0.0 else 1.0, 0.0, 0.0)
    elif near == y[0]:
        n = (0.0, -1.0 if d[1] > 0.0 else 1.0, 0.0)
    else:
        n = (0.0, 0.0, -1.0 if d[2] > 0.0 else 1.0)
    return (near, n, 1)


def scene(o, d):
    h = wall_x(o, d, -1.0, 2)
    h = closer(h, wall_x(o, d, 1.0, 3))
    h = closer(h, wall_y(o, d, -1.0))
    h = closer(h, wall_y(o, d, 1.0))
    h = closer(h, back_wall(o, d))
    h = closer(h, sphere(o, d, (-0.42, -0.6, 0.3), 0.4, 5))
    h = closer(h, sphere(o, d, (0.45, -0.65, -0.35), 0.35, 6))
    return closer(h, block(o, d, (0.1, -1.0, 0.25), (0.7, 0.25, 0.8)))


def passes(p, d, dist):
    h = scene(p, d)
    if h[0] >= dist or h[2] == 4:
        return 1.0
    if h[2] == 6:
        t = h[0] + eps()
        return 0.85 * passes(add(p, scale(d, t)), d, dist - t)
    return 0.0


# ---- the path ----

# the emitter's radiance; it is the square |x|, |z| < 0.25 at y = 1
def emission():
    return 17.0


# direct light at p (normal n) from one random point of the emitter,
# divided by the pdf of that point (1 / area)
def next_event(p, n, h):
    q = (unit_float(hash(h + 1)) * 0.5 - 0.25, 0.999, unit_float(hash(h + 2)) * 0.5 - 0.25)
    l = sub(q, p)
    d2 = dot(l, l)
    dist = sqrt(d2)
    ld = scale(l, 1.0 / dist)
    cos = dot(n, ld)
    if cos <= 0.0 or ld[1] <= 0.0:
        return 0.0
    return emission() * passes(add(p, scale(n, eps())), ld, dist - 0.01) * cos * ld[1] * 0.25 / (d2 * 3.14159265)


# a cosine-weighted direction around n: a uniform point of the unit disk
# lifted to the hemisphere (Malley), in a frame built from n (Frisvad,
# "Building an Orthonormal Basis from a 3D Unit Vector Without
# Normalization", 2012)
def disk(h):
    x = unit_float(hash(h)) * 2.0 - 1.0
    y = unit_float(hash(h + 1)) * 2.0 - 1.0
    if x * x + y * y < 1.0:
        return (x, y)
    return disk(hash(h + 2))


def bounce(n, h):
    u = disk(h)
    z = sqrt(fmax(0.0, 1.0 - u[0] * u[0] - u[1] * u[1]))
    if n[2] < -0.9999:
        t = (0.0, -1.0, 0.0)
        b = (-1.0, 0.0, 0.0)
    else:
        a = 1.0 / (1.0 + n[2])
        c = -n[0] * n[1] * a
        t = (1.0 - n[0] * n[0] * a, c, -n[0])
        b = (c, 1.0 - n[1] * n[1] * a, -n[1])
    return add(add(scale(t, u[0]), scale(b, u[1])), scale(n, z))


def glass(p, d, n, depth, h, seen):
    cosi = -dot(d, n)
    eta = 1.0 / 1.5
    if cosi < 0.0:
        n = scale(n, -1.0)
        cosi = -cosi
        eta = 1.5
    k = 1.0 - eta * eta * (1.0 - cosi * cosi)
    f = 1.0
    if k >= 0.0:
        c = 1.0 - fmin(cosi, sqrt(k))
        f = 0.04 + 0.96 * c * c * c * c * c
    if unit_float(hash(h + 7)) < f:
        return path(add(p, scale(n, eps())), reflect(d, n), depth - 1, hash(h + 8), seen)
    rd = add(scale(d, eta), scale(n, eta * cosi - sqrt(k)))
    return path(sub(p, scale(n, eps())), rd, depth - 1, hash(h + 8), seen)


# the light arriving at o from direction d; seen = 1 when emission counts
# here: a camera ray, and mirror or glass after it. After a diffuse
# bounce the emitter is reached by next-event estimation only (caustics
# are left out; counting them would add light twice).
def path(o, d, depth, h, seen):
    x = scene(o, d)
    if x[0] >= far() or depth == 0:
        return (0.0, 0.0, 0.0)
    n = x[1]
    m = x[2]
    p = add(o, scale(d, x[0]))
    if m == 4:
        e = emission() if seen == 1 else 0.0
        return (e, e, e)
    if m == 5:
        return scale(path(add(p, scale(n, eps())), reflect(d, n), depth - 1, hash(h + 3), seen), 0.9)
    if m == 6:
        return glass(p, d, n, depth, h, seen)
    direct = next_event(p, n, h)
    more = path(add(p, scale(n, eps())), bounce(n, hash(h + 4)), depth - 1, hash(h + 5), 0)
    return mul(albedo(m), add((direct, direct, direct), more))


# ---- camera and image ----

def pixel(x, y):
    w = f32(size())
    c = (0.0, 0.0, 0.0)
    for s in range(spp()):
        h = hash(hash(y * size() + x) + s * 7919)
        px = f32(x) + unit_float(hash(h + 11))
        py = f32(y) + unit_float(hash(h + 12))
        d = unit((2.0 * px / w - 1.0, 1.0 - 2.0 * py / w, 2.2))
        c = add(c, path((0.0, 0.0, -3.2), d, 6, h, 1))
    c = scale(c, 1.0 / f32(spp()))
    return (channel(c[0]) << 16) | (channel(c[1]) << 8) | channel(c[2])


# display: a soft shoulder (c / (1 + c)) with gain, then gamma 2
def channel(c):
    v = 1.6 * c / (1.0 + c)
    return int(sqrt(fmin(fmax(v, 0.0), 1.0)) * 255.0 + 0.5)


def cols(y, x0, n):
    if n == 1:
        return pixel(x0, y)
    h = n // 2
    return (cols(y, x0, h), cols(y, x0 + h, n - h))


def rows(y0, n):
    if n == 1:
        return cols(y0, 0, size())
    h = n // 2
    return (rows(y0, h), rows(y0 + h, n - h))


def main():
    return (size(), size(), rows(0, size()))
