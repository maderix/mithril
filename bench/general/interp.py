# A small expression-language interpreter: an AST with many constructor
# kinds, an environment as an association list, and a let/if/arith
# evaluator run over generated programs. Dispatch-heavy, non-linear env.
@data
class E:
    Lit: (n,)
    Var: (i,)
    Add: (a, b)
    Mul: (a, b)
    Sub: (a, b)
    Lt: (a, b)
    If: (c, t, f)
    Let: (i, v, b)


@data
class Env:
    Emp: ()
    Bind: (i, v, rest)


def look(env, i):
    match env:
        case Emp():
            return 0
        case Bind(j, v, rest):
            if i == j:
                return v
            return look(rest, i)


def ev(e, env):
    match e:
        case Lit(n):
            return n
        case Var(i):
            return look(env, i)
        case Add(a, b):
            return (ev(a, env) + ev(b, env)) & 1048575
        case Mul(a, b):
            return (ev(a, env) * ev(b, env)) & 1048575
        case Sub(a, b):
            return (ev(a, env) + 1048576 - ev(b, env)) & 1048575
        case Lt(a, b):
            return 1 if ev(a, env) < ev(b, env) else 0
        case If(c, t, f):
            if ev(c, env) != 0:
                return ev(t, env)
            return ev(f, env)
        case Let(i, v, b):
            return ev(b, Bind(i, ev(v, env), env))


def gen(d, x, nv):
    y = (x * 1103515245 + 12345) & 2147483647
    k = (y >> 8) % 8
    if d == 0:
        if (k & 1) == 0:
            return Lit(y & 255)
        return Var((y >> 4) % (nv + 1))
    a = gen(d - 1, y, nv)
    b = gen(d - 1, y ^ 99991, nv)
    if k == 0:
        return Add(a, b)
    if k == 1:
        return Mul(a, b)
    if k == 2:
        return Sub(a, b)
    if k == 3:
        return If(Lt(a, b), a, b)
    if k == 4:
        return Let(nv + 1, a, b)
    return Add(a, Lit(k))


def run(n):
    s = 0
    for i in range(n):
        e = gen(14, i * 7919 + 1, 3)
        env = Bind(0, i & 1023, Bind(1, 7, Bind(2, i & 15, Bind(3, 3, Emp()))))
        s = (s + ev(e, env)) & 4294967295
    return s


def main():
    return run(3000)
