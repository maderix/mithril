# `return ()` in a dive tail with no native return arity (Ok([]) before)
def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)


def unit(n):
    if fib(n) > 1000000:
        return ()
    return ()


def main():
    return unit(30)
