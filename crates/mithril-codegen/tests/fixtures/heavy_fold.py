# a proven fold whose iterations are heavy (each runs an inner loop of
# thousands of steps): the split must follow measured work, not the
# iteration count
def heavy(t):
    x = t * 2654435761 + 1
    for k in range(3000):
        x = (x * 1103515245 + 12345 + k) & 4294967295
    return x & 65535


def main():
    s = 0
    for t in range(96):
        s = (s + heavy(t)) & 4294967295
    return s
