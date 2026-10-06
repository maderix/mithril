# Small specialization example used by docs/guide.html.
def square(x):
    return x * x


def pick(c, a, b):
    if c > 0:
        return a
    return b


def shade(n):
    return square(n) + pick(1, square(3), n)


def main():
    return shade(array_len(array_new(5, 0)))
