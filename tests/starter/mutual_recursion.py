def is_even(n):
    if n == 0:
        return True
    return is_odd(n - 1)


def is_odd(n):
    if n == 0:
        return False
    return is_even(n - 1)


def main():
    if is_even(10) and is_odd(7):
        return 1
    return 0
