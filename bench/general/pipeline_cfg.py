# A pipeline specialized by a runtime configuration: one stage closure per
# config entry, each with a setup cost (a table folded from its config),
# the stage list applied to n inputs. The setup runs once per stage and is
# shared by every input; a strict evaluation reruns it per input.
@data
class L:
    Nil: ()
    Cons: (h, t)


def setup(c):
    s = 0
    for i in range(20000):
        s = (s + ((i * c + 7) & 4095)) & 1048575
    return s


def stage(c):
    return lambda x: (x * 3 + setup(c)) & 1048575


def stages(cfg):
    match cfg:
        case Nil():
            return Nil()
        case Cons(c, rest):
            return Cons(stage(c), stages(rest))


def apply_all(fs, x):
    match fs:
        case Nil():
            return x
        case Cons(f, rest):
            return apply_all(rest, f(x))


def go(fs, i, acc):
    if i == 0:
        return acc
    return go(fs, i - 1, (acc + apply_all(fs, i)) & 1048575)


def run(n):
    b = array_len(array_new(3, 0))
    cfg = Cons(b, Cons(b + 1, Cons(b + 2, Cons(b + 3, Nil()))))
    return go(stages(cfg), n, 0)


def main():
    return run(20000)
