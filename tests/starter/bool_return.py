def is_even(n):
    return n % 2 == 0


def main():
    c = 0
    for i in range(10):
        if is_even(i):
            c = c + 1
    return c
