# a function whose only floats are parameters is not a native scalar form:
# native code would do int arithmetic on the float ports (returned 0)
def dbl(x, n):
    if n == 0:
        return x
    return dbl(x + x, n - 1)
def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)
def main():
    return dbl(1.5, fib(20) & 3)
