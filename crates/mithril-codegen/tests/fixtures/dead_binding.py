def spin(n):
    return spin(n + 1)


def boom(n):
    return n // 0


def main():
    x = spin(0)
    y = 1 // 0
    z = boom(3) + spin(1)
    f = lambda q: spin(q)
    return 5
