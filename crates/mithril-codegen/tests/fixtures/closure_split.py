# A split loop whose leaves share a list of closures: every leaf copies the
# closures out of the shared list and applies its copies, on whichever
# worker runs it. Copies of one closure share its body until applied, so
# the workers rewrite shared net structure concurrently.
@data
class L:
    Nil: ()
    Cons: (h, t)


def stage(c):
    return lambda x: (x * 3 + c) & 1048575


def apply_all(fs, x):
    match fs:
        case Nil():
            return x
        case Cons(f, rest):
            return apply_all(rest, f(x))


def go(fs, n):
    acc = 0
    for i in range(n):
        acc = (acc + apply_all(fs, n - i)) & 1048575
    return acc


def main():
    b = array_len(array_new(3, 0))
    return go(Cons(stage(b), Cons(stage(b + 1), Nil())), 10000)
