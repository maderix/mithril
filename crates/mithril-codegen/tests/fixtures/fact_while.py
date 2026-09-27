def fact(n):
    r = 1
    while n > 0:
        r = r * n
        n = n - 1
    return r

def spin(n):
    s = 0
    while n > 0:
        s = s + n * n
        n = n - 1
    return s

def main():
    return fact(18) + spin(9000)
