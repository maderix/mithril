def main():
    t = 0
    for i in range(100):
        if i % 3 == 0:
            continue
        if i > 20:
            break
        t = t + i
    return t
