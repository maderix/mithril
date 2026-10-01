"""Independent exhaustive reference for the demo's explicit compiler contract."""

import itertools

OPS = {"add": 0, "mul": 1, "xor": 2, "div_odd": 3}
DEFAULTS = dict(length=255, scratch=4096, sram=2048, transfer_price=2, tiles=(16, 64))


def contract(graph, supplied):
    if type(graph.get("inputs")) is not int or not 1 <= graph["inputs"] <= 4:
        raise ValueError("inputs must be 1..4")
    nodes = graph.get("nodes", [])
    if not 1 <= len(nodes) <= 5:
        raise ValueError("the demo supports 1..5 operations")
    for i, node in enumerate(nodes):
        if len(node) != 3 or node[0] not in OPS:
            raise ValueError("expected a supported opcode and two references")
        if any(type(r) is not int or not -graph["inputs"] <= r < i for r in node[1:]):
            raise ValueError("references must name inputs or earlier nodes")
    outputs = graph.get("outputs", [])
    if (
        not outputs
        or len(set(outputs)) != len(outputs)
        or any(type(i) is not int or not 0 <= i < len(nodes) for i in outputs)
    ):
        raise ValueError("outputs must name distinct operations")
    reachable = set(outputs)
    for i in reversed(range(len(nodes))):
        if i in reachable:
            reachable.update(r for r in nodes[i][1:] if r >= 0)
    if len(reachable) != len(nodes):
        raise ValueError("remove dead operations before planning")
    config = dict(DEFAULTS, **supplied)
    for name in ("length", "scratch", "sram", "transfer_price"):
        if type(config[name]) is not int or not 0 <= config[name] <= 1000000:
            raise ValueError(f"{name} must be an integer in 0..1000000")
    tiles = config["tiles"]
    if (
        not tiles
        or len(set(tiles)) != len(tiles)
        or any(type(t) is not int or t < 16 or t > 256 or t % 16 for t in tiles)
    ):
        raise ValueError("tiles must be distinct multiples of 16 in 16..256")
    return config


def regions(n, mask):
    start = 0
    for i in range(n):
        if i == n - 1 or mask & (1 << i):
            yield start, i + 1
            start = i + 1


def evaluate(graph, supplied, mask, placement, tile):
    c = contract(graph, supplied)
    n = len(graph["nodes"])
    if (
        not 0 <= mask < 1 << (n - 1)
        or not 0 <= placement < 1 << n
        or tile not in c["tiles"]
    ):
        raise ValueError("invalid candidate")
    full, tail = divmod(c["length"], tile)
    blocks = full + bool(tail)
    score, transfers, materialized = 0, 0, set()
    decisions = []
    for start, end in regions(n, mask):
        members = set(range(start, end))
        assigned = {bool(placement & (1 << i)) for i in members}
        if len(assigned) != 1:
            return None
        accelerated = assigned.pop()
        external = {
            r
            for node in graph["nodes"][start:end]
            for r in node[1:]
            if r not in members
        }
        outside = set(graph["outputs"])
        for i, node in enumerate(graph["nodes"]):
            if i not in members:
                outside.update(node[1:])
        exported = members & outside
        weight = sum(
            8 if node[0] == "div_odd" else 1 for node in graph["nodes"][start:end]
        )
        vectorable = all(node[0] != "div_odd" for node in graph["nodes"][start:end])
        local_bytes = (len(external) + len(members)) * tile * 4 if accelerated else 0
        if accelerated and (not vectorable or local_bytes > c["sram"]):
            return None
        width = 16 if accelerated else 4 if vectorable else 1
        iterations = full * (tile // width) + tail // width + tail % width
        cross = sum(
            accelerated != (r >= 0 and bool(placement & (1 << r))) for r in external
        )
        cross += len(exported & set(graph["outputs"])) if accelerated else 0
        score += iterations * (weight + len(external) + len(exported))
        score += blocks * ((8 if accelerated else 4) + cross * c["transfer_price"])
        transfers += cross
        materialized.update(exported - set(graph["outputs"]))
        decisions.append(
            dict(
                start=start,
                end=end,
                device="accelerator" if accelerated else "cpu",
                width=width,
                sram=local_bytes,
                transfers=cross,
            )
        )
    scratch = len(materialized) * tile * 4
    if scratch > c["scratch"]:
        return None
    return dict(
        key=[score, scratch, transfers, mask, placement, tile], regions=decisions
    )


def solve(graph, supplied):
    c = contract(graph, supplied)
    n = len(graph["nodes"])
    candidates = (
        evaluate(graph, c, mask, placement, tile)
        for mask, placement, tile in itertools.product(
            range(1 << (n - 1)), range(1 << n), sorted(c["tiles"])
        )
    )
    return min((p for p in candidates if p is not None), key=lambda p: p["key"])
