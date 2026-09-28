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
    }
    exec(code, g)
    print(g["main"]())


if __name__ == "__main__":
    sys.setrecursionlimit(100000)
    run(sys.argv[1])
