def fact(n):
    r = 1
    while n > 0:
        r = r * n
        n = n - 1
    return r

def main():
    return fact(10)
