# A random graph as an array of adjacency lists (boxed elements) and an
# int visited array; iterative DFS with an explicit stack list, then a
# component count. Mixes boxed and int arrays with irregular access.
@data
class L:
    Nil: ()
    Cons: (h, t)


def build(n, m, x):
    adj = array_new(n, Nil())
    for i in range(m):
        x = (x * 1103515245 + 12345) & 2147483647
        u = x % n
        x = (x * 1103515245 + 12345) & 2147483647
        v = x % n
        adj = array_set(adj, u, Cons(v, array_get(adj, u)))
        adj = array_set(adj, v, Cons(u, array_get(adj, v)))
    return adj


def push_all(l, stack):
    match l:
        case Nil():
            return stack
        case Cons(h, t):
            return push_all(t, Cons(h, stack))


def dfs(adj, seen, stack, cnt):
    match stack:
        case Nil():
            return (seen, cnt)
        case Cons(u, rest):
            if array_get(seen, u) != 0:
                return dfs(adj, seen, rest, cnt)
            seen2 = array_set(seen, u, 1)
            return dfs(adj, seen2, push_all(array_get(adj, u), rest), (cnt * 31 + u) & 4294967295)


def comps(adj, n):
    seen = array_new(n, 0)
    c = 0
    h = 0
    for u in range(n):
        if array_get(seen, u) == 0:
            r = dfs(adj, seen, Cons(u, Nil()), h)
            seen = r[0]
            h = r[1]
            c = c + 1
    return (c * 1000003 + h) & 4294967295


def run(k):
    s = 0
    for r in range(k):
        adj = build(20000, 30000, r + 1)
        s = (s + comps(adj, 20000)) & 4294967295
    return s


def main():
    return run(600)
