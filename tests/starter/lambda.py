def apply(f, x):
    return f(x)


def main():
    add3 = lambda x: x + 3
    return apply(add3, 4) * 10 + apply(lambda y: y * y, 5)
