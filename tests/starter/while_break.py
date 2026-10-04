def main():
    n = 27
    steps = 0
    while True:
        if n == 1:
            break
        if n % 2 == 0:
            n = n // 2
        else:
            n = 3 * n + 1
        steps = steps + 1
    return steps
