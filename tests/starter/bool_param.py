def pick(flag, x):
    if flag:
        return x * 2
    return x + 1


def main():
    return pick(True, 10) + pick(False, 10)
