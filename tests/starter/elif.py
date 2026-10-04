def grade(s):
    if s >= 90:
        return 4
    elif s >= 80:
        return 3
    elif s >= 70:
        return 2
    else:
        return 0


def main():
    return grade(95) * 1000 + grade(85) * 100 + grade(75) * 10 + grade(10)
