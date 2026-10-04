def main():
    a, b = 0, 1
    for i in range(50):
        a, b = b, a + b
    return a
