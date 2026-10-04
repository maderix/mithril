# A field of spheres, path tracing.
#
# Forty-nine spheres stand on a checkered ground under an open sky: most
# are diffuse in hashed colours, some are mirrors and some are glass. The
# sky is an emitter that brightens toward the horizon, and a small sun
# lights the scene. A path meets up to six surfaces. At a diffuse surface
# it takes the sun's light directly (a shadow ray toward the sun) and
# continues in a cosine-weighted random direction, which may reach the sky;
# mirror and glass continue specularly (glass picks reflection or
# refraction with the Fresnel probability). The sun is counted only through
# its shadow rays after a diffuse bounce, so no light is counted twice.
#
# Each sphere's position, size, colour and material are a hash of its
# index, and randomness is a hash of (pixel, sample, bounce): the image is
# the same on every run, thread count and device.
#
# main() returns (width, height, pixels):
# `mithril run --image field.ppm demos/sphere_field.py --gpu`.


def width():
    return 1280


def height():
    return 720


def spp():
    return 64


def depth():
    return 6


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


# ---- the scene ----

def far():
    return 1e30


def eps():
    return 1e-3


def miss():
    return (far(), (0.0, 0.0, 0.0), 0, 0)


def closer(h, k):
    if k[0] < h[0]:
        return k
    return h


def rows():
    return 7


# Sphere i of rows() * rows(): a jittered grid cell, a radius, a material
# (1 diffuse, 5 mirror, 6 glass) and the hash that picks its colour.
def ball(i):
    h = hash(i * 2654435761 + 97)
    r = 0.22 + 0.2 * unit_float(hash(h + 1))
    x = (f32(i % rows()) - f32(rows() - 1) * 0.5) * 1.15 + 0.5 * (unit_float(hash(h + 2)) - 0.5)
    z = f32(i // rows()) * 1.15 - 1.0 + 0.5 * (unit_float(hash(h + 3)) - 0.5)
    k = unit_float(hash(h + 4))
    mat = 5 if k < 0.18 else (6 if k < 0.34 else 1)
    return ((x, r, z), r, mat, h)


def sphere(o, d, c, r, mat, tint):
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
    return (t, scale(sub(add(o, scale(d, t)), c), 1.0 / r), mat, tint)


def balls(o, d, i, best):
    if i == rows() * rows():
        return best
    b = ball(i)
    return balls(o, d, i + 1, closer(best, sphere(o, d, b[0], b[1], b[2], b[3])))


# the ground y = 0, material 2 (checkered)
def ground(o, d):
    if d[1] >= 0.0:
        return miss()
    t = -o[1] / d[1]
    if t < eps():
        return miss()
    return (t, (0.0, 1.0, 0.0), 2, 0)


def scene(o, d):
    return balls(o, d, 0, ground(o, d))


def albedo(m, p, tint):
    if m == 2:
        check = (int(p[0] + 1000.0) + int(p[2] + 1000.0)) % 2
        return (0.62, 0.6, 0.56) if check == 0 else (0.2, 0.2, 0.22)
    # a hashed colour, kept away from black and from full saturation
    return (0.15 + 0.7 * unit_float(hash(tint + 5)), 0.15 + 0.7 * unit_float(hash(tint + 6)), 0.15 + 0.7 * unit_float(hash(tint + 7)))


# light passing from p along d to the sky: glass dims it, anything else blocks it
def passes(p, d):
    h = scene(p, d)
    if h[0] >= far():
        return 1.0
    if h[2] == 6:
        t = h[0] + eps()
        return 0.85 * passes(add(p, scale(d, t)), d)
    return 0.0


# ---- light ----

def sun_dir():
    return (-0.42, 0.62, -0.66)


# the sky's radiance in direction d: brighter toward the horizon
def sky(d):
    s = fmax(d[1], 0.0)
    return add(scale((1.0, 1.0, 1.0), 1.0 - s), scale((0.45, 0.65, 1.0), s))


# the sun's light at p (normal n) through one shadow ray
def sun_light(p, n):
    l = unit(sun_dir())
    c = dot(n, l)
    if c <= 0.0:
        return 0.0
    return 3.2 * c * passes(add(p, scale(n, eps())), l)


# a cosine-weighted direction around n (as in cornell_path.py)
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


def glass(p, d, n, depth, h):
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
        return path(add(p, scale(n, eps())), reflect(d, n), depth - 1, hash(h + 8))
    rd = add(scale(d, eta), scale(n, eta * cosi - sqrt(k)))
    return path(sub(p, scale(n, eps())), rd, depth - 1, hash(h + 8))


# the light arriving at o from direction d
def path(o, d, depth, h):
    x = scene(o, d)
    if x[0] >= far():
        return sky(d)
    if depth == 0:
        return (0.0, 0.0, 0.0)
    n = x[1]
    m = x[2]
    p = add(o, scale(d, x[0]))
    if m == 5:
        return scale(path(add(p, scale(n, eps())), reflect(d, n), depth - 1, hash(h + 3)), 0.9)
    if m == 6:
        return glass(p, d, n, depth, h)
    direct = sun_light(p, n)
    more = path(add(p, scale(n, eps())), bounce(n, hash(h + 4)), depth - 1, hash(h + 5))
    return mul(albedo(m, p, x[3]), add((direct, direct, direct), more))


# ---- camera and image ----

def pixel(x, y):
    w = f32(width())
    hgt = f32(height())
    c = (0.0, 0.0, 0.0)
    eye = (0.0, 1.35, -6.0)
    for s in range(spp()):
        h = hash(hash(y * width() + x) + s * 7919)
        px = f32(x) + unit_float(hash(h + 11))
        py = f32(y) + unit_float(hash(h + 12))
        d = unit(((2.0 * px - w) / hgt, (hgt - 2.0 * py) / hgt - 0.12, 2.0))
        c = add(c, path(eye, d, depth(), h))
    c = scale(c, 1.0 / f32(spp()))
    return (channel(c[0]) << 16) | (channel(c[1]) << 8) | channel(c[2])


# display: a soft shoulder (c / (1 + c)) with gain, then gamma 2
def channel(c):
    v = 1.5 * c / (1.0 + c)
    return int(sqrt(fmin(fmax(v, 0.0), 1.0)) * 255.0 + 0.5)


def main():
    w = width()
    h = height()
    img = array_new(w * h, 0)
    for y in range(h):
        for x in range(w):
            img = array_set(img, y * w + x, pixel(x, y))
    return (w, h, img)
