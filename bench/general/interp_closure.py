# Closure compilation of a program only known at runtime: the AST is
# translated once into a tree of closures, then applied to n environments.
# No sharing win is expected (every part of the closures depends on the
# environment): this measures the cost of closure-heavy code on the net,
# where each application copies the closure tree by the DUP rules.
@data
class E:
    Lit: (n,)
    Var: (i,)
    Add: (a, b)
    Mul: (a, b)
    If: (c, t, f)


@data
class Env:
    Emp: ()
    Bind: (v, rest)


def look(env, i):
    match env:
        case Emp():
            return 0
        case Bind(v, rest):
            if i == 0:
                return v
            return look(rest, i - 1)


def compile(e):
    match e:
        case Lit(n):
            return lambda env: n
        case Var(i):
            return lambda env: look(env, i)
        case Add(a, b):
            ca = compile(a)
            cb = compile(b)
            return lambda env: (ca(env) + cb(env)) & 1048575
        case Mul(a, b):
            ca = compile(a)
            cb = compile(b)
            return lambda env: (ca(env) * cb(env)) & 1048575
        case If(c, t, f):
            cc = compile(c)
            ct = compile(t)
            cf = compile(f)
            return lambda env: ct(env) if cc(env) != 0 else cf(env)


def gen(d, x):
    y = (x * 1103515245 + 12345) & 2147483647
    k = (y >> 8) % 4
    if d == 0:
        if k & 1 == 0:
            return Lit(y & 255)
        return Var((y >> 4) % 3)
    a = gen(d - 1, y)
    b = gen(d - 1, y ^ 99991)
    if k == 0:
        return Add(a, b)
    if k == 1:
        return Mul(a, b)
    if k == 2:
        return If(a, b, Lit(y & 7))
    return Add(Mul(a, Lit(3)), b)


def go(p, i, acc):
    if i == 0:
        return acc
    v = p(Bind(i, Bind((i * 7) & 255, Bind(i ^ 5, Emp()))))
    return go(p, i - 1, (acc * 31 + v) & 1048575)


def run(n):
    seed = array_len(array_new(42, 0))
    return go(compile(gen(7, seed)), n, 0)


def main():
    return run(20000)
