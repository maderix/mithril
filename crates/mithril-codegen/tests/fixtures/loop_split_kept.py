# Loops the split pass must leave alone: state read by the work, a local
# used after the loop, plain arithmetic.
def f(i, k):
    return (i * 2654435761 + k) & 1048575


def reads_state(n):
    s = 1
    for i in range(n):
        t = f(i + s, 1)
        s = (s + t) & 1048575
    return s


def local_after(n):
    s = 0
    t = 0
    for i in range(n):
        t = f(i, 2)
        s = (s * 3 + t) & 1048575
    return s + t


def arithmetic(n):
    s = 0
    for i in range(n):
        s = (s * 5 + i) & 1048575
    return s


def main():
    return reads_state(60) + local_after(40) + arithmetic(70)
