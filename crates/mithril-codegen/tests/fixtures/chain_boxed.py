# A sequential chain of boxed values: non-tail recursion of depth n with no
# fork anywhere (every step waits for the one below it). On the device it
# must not cost a round per step (a cut runs inline with the budget in the
# parallel world), and the budget bounds the native recursion depth.
@data
class Box:
    B: (v,)


def count(n):
    if n == 0:
        return B(0)
    match count(n - 1):
        case B(v):
            return B((v * 3 + 1) & 4294967295)


def main():
    match count(array_len(array_new(100000, 0))):
        case B(v):
            return v
