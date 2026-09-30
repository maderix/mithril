@data
class T:
    A: ()
    B: (v)
    C: (v)


def pick(i):
    if i % 3 == 0:
        return A()
    if i % 3 == 1:
        return B(i)
    return C(i)


def f(x):
    s = 0
    if x > 0:
        if x == 100:
            return 0
        s = s + 0
        t0 = 1
    else:
        t0 = 2
    if x > 1:
        if x == 101:
            return 1
        s = s + 1
        t1 = 1
    else:
        t1 = 2
    if x > 2:
        if x == 102:
            return 2
        s = s + 2
        t2 = 1
    else:
        t2 = 2
    if x > 3:
        if x == 103:
            return 3
        s = s + 3
        t3 = 1
    else:
        t3 = 2
    if x > 4:
        if x == 104:
            return 4
        s = s + 4
        t4 = 1
    else:
        t4 = 2
    if x > 5:
        if x == 105:
            return 5
        s = s + 5
        t5 = 1
    else:
        t5 = 2
    if x > 6:
        if x == 106:
            return 6
        s = s + 6
        t6 = 1
    else:
        t6 = 2
    if x > 7:
        if x == 107:
            return 7
        s = s + 7
        t7 = 1
    else:
        t7 = 2
    if x > 8:
        if x == 108:
            return 8
        s = s + 8
        t8 = 1
    else:
        t8 = 2
    if x > 9:
        if x == 109:
            return 9
        s = s + 9
        t9 = 1
    else:
        t9 = 2
    if x > 10:
        if x == 110:
            return 10
        s = s + 10
        t10 = 1
    else:
        t10 = 2
    if x > 11:
        if x == 111:
            return 11
        s = s + 11
        t11 = 1
    else:
        t11 = 2
    if x > 12:
        if x == 112:
            return 12
        s = s + 12
        t12 = 1
    else:
        t12 = 2
    if x > 13:
        if x == 113:
            return 13
        s = s + 13
        t13 = 1
    else:
        t13 = 2
    match pick(x):
        case A():
            return s
        case B(v):
            u = v
        case C(v):
            u = 0 - v
    return s + u + t0 + t1 + t2 + t3 + t4 + t5 + t6 + t7 + t8 + t9 + t10 + t11 + t12 + t13


def main():
    return (f(3), f(9), f(20), f(105), f(0))
