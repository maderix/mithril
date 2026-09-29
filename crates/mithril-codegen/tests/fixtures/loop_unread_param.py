# a loop-form dive frees an owned parameter its body never reads on every
# iteration (leaked 2 cells per iteration before)
@data
class L:
    Nil: ()
    Cons: (h, t)


def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)


def spin(i, junk, acc):
    if i == 0:
        return acc
    return spin(i - 1, Cons(i, Cons(i, Nil())), acc + i)


def main():
    return spin(200000 + fib(3), Nil(), 0)
