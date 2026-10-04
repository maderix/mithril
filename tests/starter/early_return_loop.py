def first_square_over(n):
    for i in range(100):
        if i * i > n:
            return i
    return -1


def main():
    return first_square_over(50)
