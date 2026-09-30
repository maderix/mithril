# A runtime result containing every heap-backed value kind, wide chains,
# shared values, and arrays that lead back to cells.
@data
class Box:
    Empty: ()
    Pair: (a, b)
    Wide: (a, b, c, d, e)


def main():
    n = array_len(array_new(3, 0))
    ints = array_set(array_new(n, -7), 1, n)
    floats = array_new(n, 2.5)
    boxes = array_new(n, Pair(n, Empty()))
    arrays = array_new(n, ints)
    wide = Wide(n, floats, boxes, arrays, Empty())
    return (wide, wide, ints, -3.5)
