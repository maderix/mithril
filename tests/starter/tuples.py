def divmod2(a, b):
    return (a // b, a % b)


def main():
    q, r = divmod2(47, 5)
    t = (1, 2, 3)
    return q * 100 + r * 10 + t[2]
