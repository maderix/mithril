# Arithmetic expression trees with shared subexpressions and variable input.
@data
class Expr:
    Lit: (value,)
    Var: ()
    Add: (left, right)
    Sub: (left, right)


def eval(t, x):
    match t:
        case Lit(value):
            return value
        case Var():
            return x
        case Add(left, right):
            return eval(left, x) + eval(right, x)
        case Sub(left, right):
            return eval(left, x) - eval(right, x)


def build(depth, seed):
    if depth == 0:
        return Sub(Lit(seed), Var())
    child = build(depth - 1, seed * 3 + 7)
    return Add(child, Sub(child, Lit(depth)))


def main():
    depth = array_len(array_new(6, 0))
    t = build(depth, -1099511627776)
    return eval(t, -17) + eval(t, 4294967295)
