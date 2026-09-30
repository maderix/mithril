# Probe p6 (absorb net-reassoc): a closure whose body diverges (spin(0)) is
# built but never applied (y = big(30000) = 0 <= 100).
# Correct: 0  -- by hand (big(30000) = 0, keep returns y); the reference interpreter
# agrees. The net lazy semantics: the closure body is never evaluated.

def spin(n):
    if n < 0:
        return 0
    return spin(n + 1)

def keep(f, y):
    if y > 100:
        return f(y)
    return y

def big(n):
    a = 0
    i = 0
    while i < n:
        a = (a + i) & 3
        i = i + 1
    return a

def g(y):
    return keep(lambda x: x + spin(0), y)

def main():
    return g(big(30000))
