def f(x):
    return x * x + 1

def total32(n):
    s = 0
    for i in range(n):
        s = (s + f(i)) & 4294967295
    return s

def main():
    return total32(100000)
