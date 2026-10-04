def adder(n):
    return lambda x: x + n


def main():
    f = adder(10)
    g = adder(20)
    return f(1) + g(2)
