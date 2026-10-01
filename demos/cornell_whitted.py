# Cornell box, Whitted ray tracing.
#
# The room is the cube [-1, 1]^3 seen through its open front: a red left
# wall, a green right wall, a white floor, ceiling and back wall, and a
# square emitter in the ceiling. In it stand a mirror sphere, a glass
# sphere (refraction with Schlick's Fresnel term) and a tall diffuse
# block. Diffuse surfaces take direct light from four points on the
# emitter (soft shadow edges; glass dims the light but does not block
# it) plus a small ambient term; mirror and glass recurse up to four
# reflections or refractions. Every pixel averages 2x2 samples.
#
# The pixel loops run in parallel with no annotation. main() returns
# (width, height, pixels), pixels a row-major array of 0xRRGGBB. `mithril run --image out.ppm demos/cornell_whitted.py`.


def size():
    return 512


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


# ---- hits: (distance, normal, material) ----

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


# the wall x = c, its normal pointing into the room
def wall_x(o, d, c, mat):
    if d[0] == 0.0:
        return miss()
    t = (c - o[0]) / d[0]
    p = add(o, scale(d, t))
    if t > eps() and within(p[1], p[2]):
        return (t, (-c, 0.0, 0.0), mat)
    return miss()


# the floor (c = -1) or the ceiling (c = 1); the ceiling holds the emitter
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


# an axis-aligned block by slabs: the entry face gives the normal
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


# ---- shading ----

# how much light passes from p along d over dist: glass passes most of it
# (no caustics: the shadow ray goes straight through), anything else
# blocks it
def passes(p, d, dist):
    h = scene(p, d)
    if h[0] >= dist or h[2] == 4:
        return 1.0
    if h[2] == 6:
        t = h[0] + eps()
        return 0.85 * passes(add(p, scale(d, t)), d, dist - t)
    return 0.0


# light from one point of the emitter
def light_from(p, n, lx, lz):
    l = sub((lx, 0.999, lz), p)
    d2 = dot(l, l)
    dist = sqrt(d2)
    ld = scale(l, 1.0 / dist)
    cos = dot(n, ld)
    if cos <= 0.0:
        return 0.0
    return passes(add(p, scale(n, eps())), ld, dist - 0.01) * cos * ld[1] / d2


def direct(p, n):
    s = light_from(p, n, -0.12, -0.12) + light_from(p, n, 0.12, -0.12)
    s = s + light_from(p, n, -0.12, 0.12) + light_from(p, n, 0.12, 0.12)
    return 0.05 + s * 0.45


def glass(p, d, n, depth):
    cosi = -dot(d, n)
    eta = 1.0 / 1.5
    if cosi < 0.0:
        n = scale(n, -1.0)
        cosi = -cosi
        eta = 1.5
    k = 1.0 - eta * eta * (1.0 - cosi * cosi)
    mirror = trace(add(p, scale(n, eps())), reflect(d, n), depth - 1)
    if k < 0.0:
        return mirror
    cost = sqrt(k)
    c = fmin(cosi, cost)
    f = 0.04 + 0.96 * (1.0 - c) * (1.0 - c) * (1.0 - c) * (1.0 - c) * (1.0 - c)
    rd = add(scale(d, eta), scale(n, eta * cosi - cost))
    through = trace(sub(p, scale(n, eps())), rd, depth - 1)
    return add(scale(mirror, f), scale(through, 1.0 - f))


def trace(o, d, depth):
    h = scene(o, d)
    if h[0] >= far() or depth == 0:
        return (0.0, 0.0, 0.0)
    n = h[1]
    m = h[2]
    p = add(o, scale(d, h[0]))
    if m == 4:
        return (1.0, 1.0, 1.0)
    if m == 5:
        return scale(trace(add(p, scale(n, eps())), reflect(d, n), depth - 1), 0.9)
    if m == 6:
        return glass(p, d, n, depth)
    return scale(albedo(m), direct(p, n))


# ---- camera and image ----

def sample(px, py):
    w = f32(size())
    d = unit((2.0 * px / w - 1.0, 1.0 - 2.0 * py / w, 2.2))
    return trace((0.0, 0.0, -3.2), d, 5)


def channel(c):
    return int(sqrt(fmin(fmax(c, 0.0), 1.0)) * 255.0 + 0.5)


def pixel(x, y):
    fx = f32(x)
    fy = f32(y)
    c = add(add(sample(fx + 0.25, fy + 0.25), sample(fx + 0.75, fy + 0.25)), add(sample(fx + 0.25, fy + 0.75), sample(fx + 0.75, fy + 0.75)))
    c = scale(c, 0.25)
    return (channel(c[0]) << 16) | (channel(c[1]) << 8) | channel(c[2])


def main():
    n = size()
    img = array_new(n * n, 0)
    for y in range(n):
        for x in range(n):
            img = array_set(img, y * n + x, pixel(x, y))
    return (n, n, img)
