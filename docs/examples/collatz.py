# Collatz step counts used by docs/guide.html: each chain is serial, the
# chains for different starting numbers are independent.
def steps(n):
    count = 0
    while n != 1:
        if n % 2 == 0:
            n = n // 2
        else:
            n = 3 * n + 1
        count = count + 1
    return count


def main():
    m = array_len(array_new(300000, 0))
    total = 0
    for i in range(1, m):
        total = total + steps(i)
    return total
