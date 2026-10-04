# A spinning black hole and its accretion disk, as it would appear.
#
# Every pixel follows a light ray backward from the camera through the
# spacetime of a black hole spinning at 0.99 of the maximum (the Kerr
# metric), integrating the ray's path with fourth-order Runge-Kutta steps
# in Mino time, where the equations of motion are polynomials in r and in
# mu = cos(theta). A ray ends at the horizon, on the disk, or far away
# among the stars; rays that skim the photon sphere circle the hole many
# times first, so the work per pixel varies tenfold across the frame.
#
# The disk is thin, runs from the innermost stable circular orbit (ISCO)
# outward, and orbits at the Keplerian rate. Its temperature falls with
# radius; each photon's frequency is shifted by gravity and by the
# orbiting gas's motion (g = E_camera / E_emitter), so the disk is seen as
# a blackbody at temperature g * T: the side turning toward the camera is
# brighter and bluer, the receding side dimmer and redder. Colours come
# from Planck's law integrated against the CIE 1931 colour matching curves
# (an analytic fit), in a table built once when the program starts.
#
# The camera rides a ship descending slowly, two degrees above the disk:
# it holds a circular prograde orbit while its radius eases from 26 M
# (outside the disk, which it sees nearly edge on) down to 6 M (skimming
# it), so it moves at the orbit's speed (0.20 c, rising to 0.41 c) and
# half way around the hole over the flight. Each pixel's direction is
# Lorentz-boosted from the moving camera's frame to the frame of the
# observers the spin drags along (aberration: the sky crowds toward the
# direction of travel), and the same boost shifts every photon's frequency:
# ahead bluer and brighter, behind redder and dimmer, for the disk and for
# the stars, which are blackbodies too. As the camera circles, the hole's
# lensing sweeps across the star field.
#
# The backward ray is traced in the time-reversed spacetime (spin -a): its
# path is the real photon's path in spin +a, and the real photon's angular
# momentum is the traced one negated.
#
# The image is made in passes, each one loop over its pixels: one ray per
# pixel, kept in high dynamic range; four more rays for each pixel that
# differs sharply from a neighbour (the photon ring's thin images, distant
# disk streaks, stars); then glare, the light above a floor spread by a
# lens-like blur and added back; then a filmic tone curve.
#
# Arithmetic is binary32 throughout; the image is the same on every run,
# thread count and device. main() returns (width, height * frames, pixels):
# `mithril run --image hole.ppm demos/black_hole.py --gpu`.


def width():
    return 1280


def height():
    return 720


def frames():
    return 40


# the first frame of this strip (a flight longer than one strip is rendered
# in several)
def first_frame():
    return 0


# frames in the whole flight, and the coordinate time between frames
def flight_frames():
    return 450


def frame_dt():
    return f32(0.3)


# ---- the scene ----

def spin():
    return f32(0.99)


# cos of the camera's polar angle: a little above the disk's plane
def cam_mu():
    return f32(0.035)


# half the vertical field of view, as a tangent
def fov():
    return f32(0.72)


def disk_out():
    return f32(18.0)


def t_peak():
    return f32(9000.0)


# exposure follows the camera like an eye adapting: the disk grows brighter
# and larger as the camera closes in
def exposure(r):
    return f32(0.06) * (r / 45.0) * sqrt(r / 45.0)


# the step size, as a fraction of the ray's own scale of change
def step_size():
    return f32(0.02)


def max_steps():
    return 40000


# ---- small helpers ----

def fabs(x):
    return x if x > 0.0 else 0.0 - x


def fmin(a, b):
    return a if a < b else b


def fmax(a, b):
    return a if a > b else b


def imin(a, b):
    return a if a < b else b


def imax(a, b):
    return a if a > b else b


def iabs(a):
    return a if a > 0 else 0 - a


def floor_int(x):
    k = int(x)
    if f32(k) > x:
        return k - 1
    return k


def hash(x):
    x = ((x >> 16) ^ x) * 73244475 & 4294967295
    x = ((x >> 16) ^ x) * 73244475 & 4294967295
    return (x >> 16) ^ x


def unit_float(h):
    return f32(h >> 8) * (1.0 / 16777216.0)


def cbrt(x):
    return cbrt_from(x, f32(1.0), 0)


def cbrt_from(x, y, i):
    if i == 12:
        return y
    return cbrt_from(x, y - (y * y * y - x) / (3.0 * y * y), i + 1)


# 2^k for an integer k
def pow2(k):
    if k == 0:
        return f32(1.0)
    if k > 0:
        return 2.0 * pow2(k - 1)
    return 0.5 * pow2(k + 1)


# e^x: 2^k * 2^f with f in [0, 1) by a degree-6 polynomial
def exp(x):
    y = x * 1.442695
    k = floor_int(y)
    f = y - f32(k)
    p = 1.0 + f * (0.6931472 + f * (0.2402265 + f * (0.05550411 + f * (0.009618129 + f * (0.001333355 + f * 0.0001540353)))))
    return p * pow2(k)


# ---- colour: blackbody radiance as linear sRGB ----

def lobe(l, mu, s1, s2):
    t = (l - mu) / (s1 if l < mu else s2)
    return exp(-0.5 * t * t)


# CIE 1931 x, y, z at wavelength l (nm), the Wyman-Sloan-Shirley fit
def cie(l):
    x = 1.056 * lobe(l, 599.8, 37.9, 31.0) + 0.362 * lobe(l, 442.0, 16.0, 26.7) - 0.065 * lobe(l, 501.1, 20.4, 26.2)
    y = 0.821 * lobe(l, 568.8, 46.9, 40.5) + 0.286 * lobe(l, 530.9, 16.3, 31.1)
    z = 1.217 * lobe(l, 437.0, 11.8, 36.0) + 0.681 * lobe(l, 459.0, 26.0, 13.8)
    return (x, y, z)


# Planck's law at temperature kelvin, integrated over 380..770 nm
def planck(kelvin, i, x, y, z):
    if i == 40:
        return (x, y, z)
    l = 380.0 + 10.0 * f32(i)
    lm = l * 1e-9
    b = 1.191043e-16 / (lm * lm * lm * lm * lm) / (exp(1.438777e-2 / (lm * kelvin)) - 1.0)
    c = cie(l)
    return planck(kelvin, i + 1, x + b * c[0], y + b * c[1], z + b * c[2])


def blackbody(kelvin):
    s = planck(kelvin, 0, f32(0.0), f32(0.0), f32(0.0))
    x = s[0] * 1e-14
    y = s[1] * 1e-14
    z = s[2] * 1e-14
    r = fmax(3.2406 * x - 1.5372 * y - 0.4986 * z, 0.0)
    g = fmax(0.0 - 0.9689 * x + 1.8758 * y + 0.0415 * z, 0.0)
    b = fmax(0.0557 * x - 0.2040 * y + 1.057 * z, 0.0)
    return (r, g, b)


# the table: entries evenly spaced in 10^6 / kelvin from 25 (40,000 K) to
# 1000 (1,000 K), three channels each
def table_size():
    return 256


def table_entry(i):
    m = 25.0 + 975.0 * f32(i) / f32(table_size() - 1)
    return blackbody(1e6 / m)


def table_channel(j):
    e = table_entry(j // 3)
    k = j % 3
    return e[0] if k == 0 else (e[1] if k == 1 else e[2])


def colour(tab, kelvin):
    m = 1e6 / fmax(kelvin, 1000.0)
    f = fmin(fmax((m - 25.0) / 975.0, 0.0), 0.9999) * f32(table_size() - 1)
    i = int(f)
    w = f - f32(i)
    j = 3 * i
    r = array_get(tab, j) + w * (array_get(tab, j + 3) - array_get(tab, j))
    g = array_get(tab, j + 1) + w * (array_get(tab, j + 4) - array_get(tab, j + 1))
    b = array_get(tab, j + 2) + w * (array_get(tab, j + 5) - array_get(tab, j + 2))
    return (r, g, b)


# ---- the disk ----

def isco(a):
    z1 = 1.0 + cbrt(1.0 - a * a) * (cbrt(1.0 + a) + cbrt(1.0 - a))
    z2 = sqrt(3.0 * a * a + z1 * z1)
    return 3.0 + z2 - sqrt((3.0 - z1) * (3.0 + z1 + 2.0 * z2))


# smooth value noise on an integer lattice, periodic in x
def lattice(i, j, per):
    k = ((i % per) + per) % per
    return unit_float(hash(k * 2654435761 ^ j * 40503 ^ 40503))


def vnoise(x, y, per):
    x0 = floor_int(x)
    y0 = floor_int(y)
    fx = x - f32(x0)
    fy = y - f32(y0)
    fx = fx * fx * (3.0 - 2.0 * fx)
    fy = fy * fy * (3.0 - 2.0 * fy)
    a = lattice(x0, y0, per)
    b = lattice(x0 + 1, y0, per)
    c = lattice(x0, y0 + 1, per)
    d = lattice(x0 + 1, y0 + 1, per)
    lo = a + (b - a) * fx
    hi = c + (d - c) * fx
    return lo + (hi - lo) * fy


# streaks sheared by the differential rotation, four octaves
def turbulence(u, rc):
    q = sqrt(rc)
    n = 0.5 * vnoise(u * 48.0, q * 20.0, 48) + 0.25 * vnoise(u * 96.0, q * 40.0, 96)
    n = n + 0.125 * vnoise(u * 192.0, q * 80.0, 192) + 0.0625 * vnoise(u * 384.0, q * 160.0, 384)
    return 0.45 + 1.1 * n


# the light the disk sends at radius rc and angle pc along a ray whose
# energy at the camera is ef (per unit energy far away) and whose traced
# angular momentum is l, at time t
def disk(tab, rc, pc, ef, l, t):
    a = spin()
    rin = isco(a)
    s = sqrt(rc)
    r15 = rc * s
    om = 1.0 / (r15 + a)
    ut = (r15 + a) / (s * sqrt(s) * sqrt(r15 - 3.0 * s + 2.0 * a))
    g = ef / (ut * (1.0 + om * l))
    x = rin / rc
    prof = sqrt(x) * sqrt(sqrt(x)) * sqrt(sqrt(fmax(0.0, 1.0 - sqrt(x)))) / 0.488
    # the gas has turned by om * t since the frame began
    turn = (pc - om * t) / 6.2831853
    u = turn - f32(floor_int(turn))
    c = colour(tab, g * t_peak() * prof)
    # t_peak sets the colour, not the brightness: the unshifted disk at
    # t_peak is as luminous as the table entry of a 20,000 K one (91.5)
    n = colour(tab, t_peak())
    k = turbulence(u, rc) * 91.5 / (0.2126 * n[0] + 0.7152 * n[1] + 0.0722 * n[2])
    return (c[0] * k, c[1] * k, c[2] * k)


# ---- the sky ----

def star(fx, fy, cx, cy, sinth, g, tab):
    if cy < 0 or cy >= 1800:
        return (f32(0.0), f32(0.0), f32(0.0))
    x = ((cx % 3600) + 3600) % 3600
    h = hash(x * 2654435761 ^ (cy * 40503 + 7))
    if (h & 1023) >= 9:
        return (f32(0.0), f32(0.0), f32(0.0))
    dx = (fx - (f32(cx) + unit_float(hash(h + 1)))) * sinth
    dy = fy - (f32(cy) + unit_float(hash(h + 2)))
    d2 = dx * dx + dy * dy
    if d2 > 0.36:
        return (f32(0.0), f32(0.0), f32(0.0))
    q = 1.0 - d2 / 0.36
    u = unit_float(hash(h + 3))
    u2 = u * u
    mag = u2 * u2 * u2 * u2 * 120.0 + 1.2
    kelvin = 3000.0 + 9000.0 * unit_float(hash(h + 4))
    c0 = colour(tab, kelvin)
    n = fmax(c0[0], fmax(c0[1], c0[2])) + 1e-9
    # seen as a blackbody at g times its temperature, brighter as g grows
    c = colour(tab, g * kelvin)
    w = mag * q * q / n
    return (c[0] * w, c[1] * w, c[2] * w)


# the stars in direction (mu, phi) of the far field, their light shifted by
# g on the way to the camera: cells of about 0.1 degree, a star in one cell
# of a hundred
def sky(mu, ph, shift, tab):
    turn = ph / 6.2831853
    u = turn - f32(floor_int(turn))
    fx = u * 3600.0
    fy = (mu + 1.0) * 900.0
    cx = floor_int(fx)
    cy = floor_int(fy)
    sinth = sqrt(fmax(1e-6, 1.0 - mu * mu))
    r = f32(0.0)
    g = f32(0.0)
    b = f32(0.0)
    for k in range(9):
        s = star(fx, fy, cx + k % 3 - 1, cy + k // 3 - 1, sinth, shift, tab)
        r = r + s[0]
        g = g + s[1]
        b = b + s[2]
    # a faint mottled band of unresolved stars across the sky, light near
    # 6,500 K shifted like the rest
    z = (mu + 0.12) / 0.06
    across = 1.0 / (1.0 + z * z)
    mottle = 0.35 + 1.5 * vnoise(u * 360.0, mu * 60.0, 360)
    c = colour(tab, shift * 6500.0)
    w = 0.035 * across * across * mottle / 3.45
    return (r + c[0] * w, g + c[1] * w, b + c[2] * w)


# ---- the camera's flight ----

# a ship descending on circular prograde orbits two degrees above the disk,
# its radius easing from 26 M (outside the disk) to 6 M over the flight; the
# radial drift is its own thrust
def cam_from():
    return f32(26.0)


def cam_to():
    return f32(6.0)


def cam_radius(f):
    u = f / f32(flight_frames())
    return cam_from() + (cam_to() - cam_from()) * u * u * (3.0 - 2.0 * u)


# the azimuth at frame f: the orbit's angular velocity 1 / (r^1.5 + a)
# summed over the frames before it, each at its midpoint radius
def cam_azimuth(f, k, ph):
    if k == f:
        return ph
    r = cam_radius(f32(k) + 0.5)
    return cam_azimuth(f, k + 1, ph + frame_dt() / (r * sqrt(r) + spin()))


# the camera at frame f: radius, azimuth, and its velocity (radial,
# azimuthal) as the dragged observers measure it
def camera(f):
    a = spin()
    r = cam_radius(f32(f))
    d = r * r - 2.0 * r + a * a
    big = (r * r + a * a) * (r * r + a * a) - a * a * d
    alpha = sqrt(r * r * d / big)
    varpi = sqrt(big) / r
    om = 2.0 * a * r / big
    u = f32(f) / f32(flight_frames())
    drdt = (cam_to() - cam_from()) * 6.0 * u * (1.0 - u) / (f32(flight_frames()) * frame_dt())
    vr = r / sqrt(d) * drdt / alpha
    vp = varpi * (1.0 / (r * sqrt(r) + a) - om) / alpha
    return (r, cam_azimuth(f, 0, f32(0.0)), vr, vp)


def camera_channel(j):
    c = camera(first_frame() + j // 4)
    k = j % 4
    return c[0] if k == 0 else (c[1] if k == 1 else (c[2] if k == 2 else c[3]))


# ---- the ray ----

# the motion of the backward ray (spin -a): R'(r) / 2, M'(mu) / 2, dphi
def accel(r, mu, a, l, q, k):
    a2 = a * a
    d = r * r - 2.0 * r + a2
    w = r * r + a2 - a * l
    ar = 2.0 * r * w - (r - 1.0) * k
    am = mu * (a2 * (1.0 - 2.0 * mu * mu) - q - l * l)
    sin2 = fmax(1.0 - mu * mu, 1e-6)
    dph = 0.0 - (a - l / sin2) + a * w / d
    return (ar, am, dph)


# one fourth-order Runge-Kutta step of the state (r, mu, phi, vr, vmu)
def rk4(s, a, l, q, k):
    r = s[0]
    mu = s[1]
    vr = s[3]
    vm = s[4]
    k1 = accel(r, mu, a, l, q, k)
    h = step_size() / (fabs(vr) / r + fabs(vm) + fabs(k1[2]) / r + 1e-9)
    k2 = accel(r + 0.5 * h * vr, mu + 0.5 * h * vm, a, l, q, k)
    vr2 = vr + 0.5 * h * k1[0]
    vm2 = vm + 0.5 * h * k1[1]
    k3 = accel(r + 0.5 * h * vr2, mu + 0.5 * h * vm2, a, l, q, k)
    vr3 = vr + 0.5 * h * k2[0]
    vm3 = vm + 0.5 * h * k2[1]
    k4 = accel(r + h * vr3, mu + h * vm3, a, l, q, k)
    vr4 = vr + h * k3[0]
    vm4 = vm + h * k3[1]
    e = h / 6.0
    nr = r + e * (vr + 2.0 * vr2 + 2.0 * vr3 + vr4)
    nm = mu + e * (vm + 2.0 * vm2 + 2.0 * vm3 + vm4)
    # binary32 round-off drifts the velocities off the ray's constants,
    # which near the photon orbit scatters rays between the horizon and the
    # disk; put them back on R(r) and M(mu) away from the turning points
    w = nr * nr + a * a - a * l
    big_r = w * w - (nr * nr - 2.0 * nr + a * a) * k
    big_m = q - (q + l * l - a * a) * nm * nm - a * a * nm * nm * nm * nm
    return (nr, nm,
            s[2] + e * (k1[2] + 2.0 * k2[2] + 2.0 * k3[2] + k4[2]),
            on_shell(vr + e * (k1[0] + 2.0 * k2[0] + 2.0 * k3[0] + k4[0]), big_r),
            on_shell(vm + e * (k1[1] + 2.0 * k2[1] + 2.0 * k3[1] + k4[1]), big_m))


# v with its magnitude reset to sqrt(v2), when v2 is clear of zero
def on_shell(v, v2):
    if v2 > 0.0 and v * v > 0.25 * v2:
        return sqrt(v2) if v > 0.0 else 0.0 - sqrt(v2)
    return v


# a ray heading within half a degree of the spin axis: binary32 cannot
# resolve mu = cos(theta) there (a step's change rounds away) while the
# azimuth rate l / sin^2 shrinks the steps, so the ray would stall. Near
# the axis the ray crosses on a straight segment: it leaves at the same mu
# heading away, its azimuth advanced by pi - 2 asin(sin_min / sin), where
# sin_min^2 = l^2 / (q + l^2 + a^2) is its turning point
def pole(s, a, l, q):
    s2 = 1.0 - s[1] * s[1]
    if s2 < 1e-4 and s[1] * s[4] > 0.0:
        m = l * l / (q + l * l + a * a)
        e = fmax(s2, m)
        x = fmin(sqrt(m / e), 1.0)
        asin = 1.5707963 - sqrt(1.0 - x) * (1.5707288 - x * (0.2121144 - x * (0.074261 - 0.0187293 * x)))
        turn = 3.1415927 - 2.0 * asin
        mu = sqrt(1.0 - e)
        return (s[0], mu if s[1] > 0.0 else 0.0 - mu, s[2] + (turn if l > 0.0 else 0.0 - turn), s[3], 0.0 - s[4])
    return s


# follow the ray: c is the light gathered so far, tr the part of the
# background still visible through the thin outer edge of the disk
def march(s, a, l, q, k, ef, n, t, phc, tab, c, tr):
    if n == max_steps():
        return (c[0], c[1], c[2], n)
    s2 = pole(rk4(s, a, l, q, k), a, l, q)
    rh = 1.0 + sqrt(1.0 - a * a)
    if (s[1] > 0.0) != (s2[1] > 0.0):
        f = s[1] / (s[1] - s2[1])
        rc = s[0] + f * (s2[0] - s[0])
        if rc > isco(spin()) and rc < disk_out():
            e = disk(tab, rc, phc + s[2] + f * (s2[2] - s[2]), ef, l, t)
            op = fmin(1.0, (disk_out() - rc) / 4.0)
            c = (c[0] + tr * op * e[0], c[1] + tr * op * e[1], c[2] + tr * op * e[2])
            tr = tr * (1.0 - op)
            if tr < 0.001:
                return (c[0], c[1], c[2], n)
    if s2[0] < rh * 1.0005:
        return (c[0], c[1], c[2], n)
    if s2[0] > 1000.0 and s2[3] > 0.0:
        b = sky(s2[1], phc + s2[2], ef, tab)
        return (c[0] + tr * b[0], c[1] + tr * b[1], c[2] + tr * b[2], n)
    return march(s2, a, l, q, k, ef, n + 1, t, phc, tab, c, tr)


# the light reaching the camera from image-plane point (x, y), and the
# steps its ray took
def ray(x, y, t, cam, tab):
    a = 0.0 - spin()
    a2 = a * a
    r0 = cam[0]
    mu0 = cam_mu()
    s0 = sqrt(1.0 - mu0 * mu0)
    sig = r0 * r0 + a2 * mu0 * mu0
    del0 = r0 * r0 - 2.0 * r0 + a2
    big = (r0 * r0 + a2) * (r0 * r0 + a2) - a2 * del0 * s0 * s0
    rho = sqrt(sig)
    varpi = sqrt(big / sig) * s0
    alpha = sqrt(sig * del0 / big)
    omega = 2.0 * a * r0 / big
    # the camera's velocity relative to the dragged observers
    br = cam[2]
    bp = cam[3]
    b2 = br * br + bp * bp + 1e-12
    gam = 1.0 / sqrt(1.0 - b2)
    # the camera aims at the hole: its view axis is the direction the
    # dragged observers call straight inward, as the moving camera sees it
    # (that direction boosted by -v)
    ib = 0.0 - br
    ik = (gam - 1.0) * ib / b2 + gam
    ie = gam * (1.0 + ib)
    lr = (br * ik - 1.0) / ie
    lp = bp * ik / ie
    # the pixel's direction in the camera frame; the photon travels the
    # other way. Boost it to the dragged observers' frame; d: the photon's
    # frequency ratio camera / them
    n = sqrt(1.0 + x * x + y * y)
    cr = (lr + x * lp) / n
    ct = (0.0 - y) / n
    cp = (lp - x * lr) / n
    bd = 0.0 - (br * cr + bp * cp)
    e = gam * (1.0 + bd)
    d = 1.0 / e
    k = (gam - 1.0) * bd / b2 + gam
    nr = (cr - br * k) / e
    nt = ct / e
    np = (cp - bp * k) / e
    ef = 1.0 / (alpha + omega * varpi * np)
    pr = ef * rho * nr / sqrt(del0)
    pt = ef * rho * nt
    l = ef * varpi * np
    q = pt * pt + mu0 * mu0 * (l * l / (s0 * s0) - a2)
    kk = q + (l - a) * (l - a)
    start = (r0, mu0, f32(0.0), del0 * pr, (0.0 - s0) * pt)
    # the camera's azimuth
    phc = cam[1]
    return march(start, a, l, q, kk, ef * d, 0, t, phc, tab, (f32(0.0), f32(0.0), f32(0.0)), f32(1.0))


# ---- the image ----

# ACES-style filmic curve, then gamma 2
def tone(c):
    v = (c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14)
    return int(sqrt(fmin(fmax(v, 0.0), 1.0)) * 255.0 + 0.5)


# the camera at frame (of this strip): radius, azimuth, velocity
def cam_at(cams, frame):
    j = 4 * frame
    return (array_get(cams, j), array_get(cams, j + 1), array_get(cams, j + 2), array_get(cams, j + 3))


# the light through point (x + 0.5 + ox, y + 0.5 + oy) of the frame,
# exposed: linear, unbounded
def radiance(x, y, ox, oy, frame, cams, tab):
    w = f32(width())
    hgt = f32(height())
    t = frame_dt() * f32(first_frame() + frame)
    cam = cam_at(cams, frame)
    px = (2.0 * (f32(x) + 0.5 + ox) / w - 1.0) * fov() * w / hgt
    py = (1.0 - 2.0 * (f32(y) + 0.5 + oy) / hgt) * fov()
    c = ray(px, py, t, cam, tab)
    k = exposure(cam[0])
    return (c[0] * k, c[1] * k, c[2] * k)


# high dynamic range colour in one integer: three channels of 21 bits,
# each sqrt(c) * 8192 (up to c = 65536; steps of about 1% at c = 1e-4)
def channel_code(c):
    return imin(int(sqrt(fmax(c, 0.0)) * 8192.0 + 0.5), 2097151)


def pack(c):
    return (channel_code(c[0]) << 42) | (channel_code(c[1]) << 21) | channel_code(c[2])


def channel_value(q):
    v = f32(q) / 8192.0
    return v * v


def unpack(p):
    return (channel_value(p >> 42), channel_value((p >> 21) & 2097151), channel_value(p & 2097151))


# the brightest channel's code, the perceptual level used to find edges
def level(p):
    return imax(p >> 42, imax((p >> 21) & 2097151, p & 2097151))


# pass 1: one ray through each pixel's centre
def first_ray(i, cams, tab):
    w = width()
    h = height()
    return pack(radiance(i % w, (i // w) % h, f32(0.0), f32(0.0), i // (w * h), cams, tab))


# pass 2: a pixel that differs sharply from a neighbour (the photon ring's
# thin images, the disk's streaks far away) takes four more rays, one in
# each quarter of the pixel, and averages all five
def refine(i, base, cams, tab):
    w = width()
    h = height()
    x = i % w
    y = (i // w) % h
    p = array_get(base, i)
    v = level(p)
    d = 0
    if x > 0:
        d = imax(d, iabs(level(array_get(base, i - 1)) - v))
    if x < w - 1:
        d = imax(d, iabs(level(array_get(base, i + 1)) - v))
    if y > 0:
        d = imax(d, iabs(level(array_get(base, i - w)) - v))
    if y < h - 1:
        d = imax(d, iabs(level(array_get(base, i + w)) - v))
    # 800 codes: a tenth of the way from black to an exposed level of 1
    if d < 800:
        return p
    c = unpack(p)
    for s in range(4):
        ox = f32(s % 2) * 0.5 - 0.25
        oy = f32(s // 2) * 0.5 - 0.25
        e = radiance(x, y, ox, oy, i // (w * h), cams, tab)
        c = (c[0] + e[0], c[1] + e[1], c[2] + e[2])
    return pack((c[0] * 0.2, c[1] * 0.2, c[2] * 0.2))


# ---- glare ----
# a lens or an eye spreads part of very bright light around it; around the
# disk's hot inner edge that spread is what makes it blaze. Light above
# glare_floor() spreads as two Gaussians, a tight core and a wider halo,
# blurred at a quarter of the resolution and added back with weight
# glare_mix(); dimmer light, and the shadow, stay sharp

def glare_floor():
    return f32(1.0)


def glare_mix():
    return f32(0.3)


# pass 3: each glare pixel is the mean of a 4 x 4 block of the image's
# light above the floor
def shrink(i, img):
    w = width()
    h = height()
    qw = w // 4
    qh = h // 4
    x = i % qw
    y = (i // qw) % qh
    o = (i // (qw * qh)) * w * h + 4 * y * w + 4 * x
    fl = glare_floor()
    c = (f32(0.0), f32(0.0), f32(0.0))
    for k in range(16):
        e = unpack(array_get(img, o + (k // 4) * w + k % 4))
        c = (c[0] + fmax(e[0] - fl, 0.0), c[1] + fmax(e[1] - fl, 0.0), c[2] + fmax(e[2] - fl, 0.0))
    return pack((c[0] / 16.0, c[1] / 16.0, c[2] / 16.0))


# pass 4 and 5: Gaussian blur of standard deviation sigma glare pixels
# along x (along_x = 1) or y (0), clamped at the frame's edges. The weight
# exp(-u^2 / 2 sigma^2) goes from tap to tap by a ratio that itself shrinks
# by exp(-1 / sigma^2) each tap
def blur(i, src, sigma, along_x):
    qw = width() // 4
    qh = height() // 4
    x = i % qw
    y = (i // qw) % qh
    rad = int(3.0 * sigma)
    s2 = 2.0 * sigma * sigma
    g = exp(f32(rad * rad) / (0.0 - s2))
    ratio = exp(f32(2 * rad - 1) / s2)
    step = exp(-2.0 / s2)
    c = (f32(0.0), f32(0.0), f32(0.0))
    tot = f32(0.0)
    for t in range(2 * rad + 1):
        u = t - rad
        if along_x == 1:
            j = i - x + imin(imax(x + u, 0), qw - 1)
        else:
            j = i + (imin(imax(y + u, 0), qh - 1) - y) * qw
        e = unpack(array_get(src, j))
        c = (c[0] + g * e[0], c[1] + g * e[1], c[2] + g * e[2])
        tot = tot + g
        g = g * ratio
        ratio = ratio * step
    return pack((c[0] / tot, c[1] / tot, c[2] / tot))


# the glare at full-resolution pixel (x, y) of a frame: bilinear in the
# glare image
def glare_at(core, halo, x, y, frame):
    qw = width() // 4
    qh = height() // 4
    fx = fmin(fmax((f32(x) + 0.5) / 4.0 - 0.5, 0.0), f32(qw - 1))
    fy = fmin(fmax((f32(y) + 0.5) / 4.0 - 0.5, 0.0), f32(qh - 1))
    x0 = int(fx)
    y0 = int(fy)
    x1 = imin(x0 + 1, qw - 1)
    y1 = imin(y0 + 1, qh - 1)
    ax = fx - f32(x0)
    ay = fy - f32(y0)
    o = frame * qw * qh
    c = (f32(0.0), f32(0.0), f32(0.0))
    for k in range(4):
        j = o + (y1 if k >= 2 else y0) * qw + (x1 if k % 2 == 1 else x0)
        g = (ax if k % 2 == 1 else 1.0 - ax) * (ay if k >= 2 else 1.0 - ay)
        e = unpack(array_get(core, j))
        f = unpack(array_get(halo, j))
        c = (c[0] + g * (e[0] + f[0]), c[1] + g * (e[1] + f[1]), c[2] + g * (e[2] + f[2]))
    return (c[0] * 0.5, c[1] * 0.5, c[2] * 0.5)


# pass 6: the image with its glare, tone mapped
def compose(i, img, core, halo):
    w = width()
    h = height()
    c = unpack(array_get(img, i))
    g = glare_at(core, halo, i % w, (i // w) % h, i // (w * h))
    m = glare_mix()
    return (tone(c[0] + m * g[0]) << 16) | (tone(c[1] + m * g[1]) << 8) | tone(c[2] + m * g[2])


def main():
    n = table_size()
    tab = array_new(3 * n, f32(0.0))
    for j in range(3 * n):
        tab = array_set(tab, j, table_channel(j))
    cams = array_new(4 * frames(), f32(0.0))
    for j in range(4 * frames()):
        cams = array_set(cams, j, camera_channel(j))
    w = width()
    h = height()
    m = w * h * frames()
    # one index per pixel of the strip of frames
    base = array_new(m, 0)
    for i in range(m):
        base = array_set(base, i, first_ray(i, cams, tab))
    img = array_new(m, 0)
    for i in range(m):
        img = array_set(img, i, refine(i, base, cams, tab))
    qm = (w // 4) * (h // 4) * frames()
    small = array_new(qm, 0)
    for i in range(qm):
        small = array_set(small, i, shrink(i, img))
    core_x = array_new(qm, 0)
    for i in range(qm):
        core_x = array_set(core_x, i, blur(i, small, f32(1.5), 1))
    core = array_new(qm, 0)
    for i in range(qm):
        core = array_set(core, i, blur(i, core_x, f32(1.5), 0))
    halo_x = array_new(qm, 0)
    for i in range(qm):
        halo_x = array_set(halo_x, i, blur(i, small, f32(6.0), 1))
    halo = array_new(qm, 0)
    for i in range(qm):
        halo = array_set(halo, i, blur(i, halo_x, f32(6.0), 0))
    out = array_new(m, 0)
    for i in range(m):
        out = array_set(out, i, compose(i, img, core, halo))
    return (w, h * frames(), out)
