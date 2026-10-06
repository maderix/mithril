# N-queens solution count used by docs/guide.html. Columns and diagonals are
# bit masks. place(row, c, ...) tries column c in this row and, separately,
# the remaining columns: two independent branches of the search.
def place(row, c, cols, d1, d2, n):
    if row == n:
        return 1
    if c == n:
        return 0
    rest = place(row, c + 1, cols, d1, d2, n)
    bit = 1 << c
    if (cols & bit) != 0 or (d1 & (1 << (row + c))) != 0 or (d2 & (1 << (row - c + n))) != 0:
        return rest
    here = place(row + 1, 0, cols | bit, d1 | (1 << (row + c)), d2 | (1 << (row - c + n)), n)
    return here + rest


def main():
    n = array_len(array_new(10, 0))
    return place(0, 0, 0, 0, 0, n)
