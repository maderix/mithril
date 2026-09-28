# Oracle shim: run a Mithril benchmark port under plain python3.
#
#   python3 bench/ports/_pyshim.py bench/ports/<name>.py
#
# Mithril ports are a Python subset plus a `@data` ADT decorator, so they
# run under CPython unchanged once `data` is provided. This runner:
#   - compiles the port with the `annotations` future flag so @data field
#     lists like `Leaf: (v,)` are never evaluated (the names are not
#     defined; Mithril's parser treats them as pure syntax),
#   - provides a `data` decorator that turns each annotation into a real
#     constructor class usable with Python 3.10 `match` statements
#     (positional patterns via __match_args__),
#   - execs the module, calls its main() and prints the returned checksum
#     (mirroring the Mithril runtime, which prints main's return value).
# Ports never import this file; it execs them byte-for-byte unchanged.
import __future__
import sys


def data(cls):
    g = sys._getframe(1).f_globals
    ann = cls.__dict__.get("__annotations__", {})
    for cname, spec in ann.items():
        body = str(spec).strip().lstrip("(").rstrip(")")
        fields = [f.strip() for f in body.split(",") if f.strip()]
        names = tuple("f%d" % k for k in range(len(fields)))

        def make(ctor_name, field_names):
            def __init__(self, *args):
                for k, v in zip(field_names, args):
                    object.__setattr__(self, k, v)

            return type(
                ctor_name,
                (),
                {
                    "__slots__": field_names,
                    "__match_args__": field_names,
                    "__init__": __init__,
                },
            )

        g[cname] = make(cname, names)
    return cls


# arrays are values: array_set returns a new array
def array_new(n, v):
    return [v] * n


def array_get(a, i):
    return a[i]


def array_set(a, i, v):
    b = list(a)
    b[i] = v
    return b


def array_len(a):
    return len(a)


# IEEE-754 binary32 on bit patterns: compute in f64 and round to f32
# (exact for + - * / sqrt: f64 has more than 2*24+2 significand bits, so
# rounding twice gives the correctly rounded binary32 result)
import math
import struct


def _f(x):
    return struct.unpack("<f", struct.pack("<I", x & 0xFFFFFFFF))[0]


def _b(v):
    try:
        return struct.unpack("<I", struct.pack("<f", v))[0]
    except OverflowError:
        return 0xFF800000 if v < 0 else 0x7F800000


def f32_add(a, b):
    return _b(_f(a) + _f(b))


def f32_sub(a, b):
    return _b(_f(a) - _f(b))


def f32_mul(a, b):
    return _b(_f(a) * _f(b))


def f32_div(a, b):
    x, y = _f(a), _f(b)
    if y == 0.0:
        if x == 0.0 or math.isnan(x):
            return 0x7FC00000
        neg = (math.copysign(1.0, x) < 0) != (math.copysign(1.0, y) < 0)
        return 0xFF800000 if neg else 0x7F800000
    return _b(x / y)


def f32_sqrt(a):
    x = _f(a)
    if x < 0 or math.isnan(x):
        return 0x7FC00000
    return _b(math.sqrt(x))


def f32_lt(a, b):
    return 1 if _f(a) < _f(b) else 0


def f32_from_u32(n):
    return _b(float(n & 0xFFFFFFFF))


def f32_to_u32(a):
    x = _f(a)
    if math.isnan(x) or x < 0 or x >= 4294967296.0:
        return 0
    return int(x)


def run(path):
    src = open(path).read()
    code = compile(
        src, path, "exec", flags=__future__.annotations.compiler_flag, dont_inherit=True
    )
    g = {
        "data": data,
        "__name__": "__mithril_port__",
        "array_new": array_new,
        "array_get": array_get,
        "array_set": array_set,
        "array_len": array_len,
        "f32_add": f32_add,
        "f32_sub": f32_sub,
        "f32_mul": f32_mul,
        "f32_div": f32_div,
        "f32_sqrt": f32_sqrt,
        "f32_lt": f32_lt,
        "f32_from_u32": f32_from_u32,
        "f32_to_u32": f32_to_u32,
    }
    exec(code, g)
    print(g["main"]())


if __name__ == "__main__":
    sys.setrecursionlimit(100000)
    run(sys.argv[1])
