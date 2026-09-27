def f(x):
    return x * x + 1

def total(n):
    s = 0
    for i in range(n):
        s = s + f(i)
    return s

def main():
    return total(100000)
