def total(n):
    if n == 0:
        return 0
    return n + total(n - 1)


def main():
    return total(100000)
