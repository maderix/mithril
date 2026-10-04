def gcd(a, b):
    while b != 0:
        a, b = b, a % b
    return a


def main():
    return gcd(1071, 462)
