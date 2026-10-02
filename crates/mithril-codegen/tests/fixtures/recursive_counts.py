# Count leaves and traversed edges of a regular recursive tree.
def solve(depth, fanout, origin, stride, span, leaves, edges):
    if depth == 0:
        return ((leaves + 1) & 4294967295, edges)
    pending = fanout
    while pending > 0:
        child = solve(depth - 1, fanout, origin + stride, stride, span,
                      leaves, (edges + 1) & 4294967295)
        leaves = child[0]
        edges = child[1]
        pending = pending - 1
    return (leaves, edges)


def main():
    depth = array_len(array_new(7, 0))
    return solve(depth, 2, 0, 1, 0, 0, 0)
