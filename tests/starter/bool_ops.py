def main():
    a = True
    b = False
    n = 0
    if a and not b:
        n = n + 1
    if a or b:
        n = n + 10
    if not (a and b):
        n = n + 100
    return n
