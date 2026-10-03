# collatz: total Collatz steps over a range of starting numbers. Each chain
# is serial (every step needs the previous number); the chains are
# independent, and the total is a proven fold (wrapping add), so the range
# splits into chunks across workers. Checksum: total steps, u32.
# Sizes: small n 1000, expect 59431; big n 3000000.


def steps(n):
    count = 0
    while n != 1:
        if n % 2 == 0:
            n = n // 2
        else:
            n = 3 * n + 1
        count = count + 1
    return count


def total(n):
    s = 0
    for i in range(1, n):
        s = (s + steps(i)) & 4294967295
    return s


def main():
    return total(3000000)
