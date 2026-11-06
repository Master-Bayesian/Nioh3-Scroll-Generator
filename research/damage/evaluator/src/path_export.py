"""Write a Compiler's programs, memory and test vectors in the flat text form the Rust runner loads.

Every operand becomes an index into one value array: constants first, then nodes. Values wider than 128 bits (a callee's return
bytes) are split into 16-byte chunks. Bytes in the input domain live in a per-candidate buffer; everything else is a shared arena.
"""
import bisect
from pathlib import Path

OPS = ["in", "mulss", "addss", "subss", "divss", "arg", "fn", "guard", "set", "cmov", "eq", "mix", "addr", "load", "comiss", "minss", "maxss",
       "sqrtss", "cvtsi2ss", "cvttss2si", "cvtss2sd", "cvtsd2ss", "and", "or", "xor", "add", "adc", "sub", "sbb", "inc", "dec", "neg", "keep_cf",
       "not", "sext", "sign", "shl", "shr", "sar", "rol", "ror", "imul", "mul", "mul:hi", "imul:lo", "imul:hi", "div", "div:hi", "idiv", "idiv:hi", "bt"]
CONDS = ["e", "ne", "b", "ae", "be", "a", "s", "ns", "l", "ge", "le", "g", "p", "np"]
ALIAS = {"z": "e", "nz": "ne", "c": "b", "nae": "b", "nb": "ae", "nc": "ae", "na": "be", "nbe": "a", "nge": "l", "nl": "ge", "ng": "le", "nle": "g",
         "pe": "p", "po": "np"}


def cond_code(m):
    c = m.removeprefix("cmov").removeprefix("set")
    if c.startswith("j"):
        c = c[1:]
    return CONDS.index(ALIAS.get(c, c))


def merged(domain):
    out = []
    for s, e in sorted(domain):
        if out and s <= out[-1][1]:
            out[-1][1] = max(out[-1][1], e)
        else:
            out.append([s, e])
    return out


class Domain:
    def __init__(self, ranges):
        self.ranges = merged(ranges)
        self.starts = [r[0] for r in self.ranges]
        self.offsets, total = [], 0
        for s, e in self.ranges:
            self.offsets.append(total)
            total += e - s
        self.size = total

    def offset(self, address):
        k = bisect.bisect_right(self.starts, address) - 1
        if k < 0 or address >= self.ranges[k][1]:
            raise ValueError(f"{address:#x} is outside the input domain")
        return self.offsets[k] + address - self.ranges[k][0]


def chunks(value, size):
    return [(value >> (128 * i)) & ((1 << 128) - 1) for i in range((size + 15) // 16)]


def lower(program, domain):
    """One program as text lines."""
    nodes = program.nodes
    pool, pool_index = [], {}

    def const(v):
        if v not in pool_index:
            pool_index[v] = len(pool)
            pool.append(v)
        return pool_index[v]

    # constants first, so node positions are known when operands are written
    for n in nodes:
        for s in n["s"]:
            if s[0] < 0:
                const(s[1])
        if n["op"] == "eq":
            const(n["expect"])
        if n["op"] == "addr":
            const(n["disp"])
        if n["op"] == "mix":
            const(n["base"] if n["w"] <= 128 else 0)  # a wider mix is only a root, rebuilt below in chunks
    width = (program.root_bytes + 15) // 16
    root_base = chunks(program.root[1], program.root_bytes) if program.root[0] < 0 else None
    if root_base:
        for v in root_base:
            const(v)
    extra_mix = []
    if program.root[0] >= 0:
        r = nodes[program.root[0]]
        parts = r.get("parts") if r["op"] == "mix" else [[program.root[0], i] for i in range(program.root_bytes)]
        base = r.get("base", 0)
        const(0)
        for k in range(width):
            extra_mix.append((chunks(base, program.root_bytes)[k], parts[16 * k:16 * k + 16]))
            const(chunks(base, program.root_bytes)[k])
    const(0)
    start, position = len(pool), []
    at = 0
    for n in nodes:
        position.append(start + at)
        at += max(1, (n["w"] + 127) // 128) if n["op"] == "fn" else 1

    def operand(s):
        return position[s[0]] if s[0] >= 0 else pool_index[s[1]]

    def part(p, dst):
        chunk = p[1] // 16 if nodes[p[0]]["op"] == "fn" else 0
        return f"{dst} {position[p[0]] + chunk} {p[1] % 16 if nodes[p[0]]['op'] == 'fn' else p[1]}"

    lines = []
    for n in nodes:
        op, s, w = n["op"], n["s"], n["w"]
        code, cond, expect = OPS.index(op), 0, 0
        a = operand(s[0]) if s and op not in ("guard", "set", "cmov", "keep_cf") else (position[s[0][0]] if s else -1)
        b = operand(s[1]) if len(s) > 1 else -1
        c, x, y, extra = -1, 0, 0, []
        if op == "in":
            x = domain.offset(n["address"])
        elif op == "load":
            x = n["size"]
        elif op == "arg":
            x = n["index"]
        elif op == "fn":
            x = n["table"]
            y = max(1, (w + 127) // 128)
            extra = [operand(q) for q in s]
        elif op in ("guard", "set"):
            cond, expect = cond_code(n["cond"]), int(n.get("expect", 0))
        elif op == "cmov":
            cond = cond_code(n["cond"])
            c = operand(s[2])
        elif op == "eq":
            b = pool_index[n["expect"]]
        elif op == "mix":
            b = pool_index[n["base"] if w <= 128 else 0]
            pieces = [part(p, k) for k, p in enumerate(n["parts"]) if p] if w <= 128 else []
            extra = [len(pieces), *pieces]
        elif op == "addr":
            b = pool_index[n["disp"]]
            a = -1
            extra = [len(s), *[f"{operand(q)} {f}" for q, f in zip(s, n["factors"])]]
        elif op in ("adc", "sbb"):
            b = operand(s[1])
            c = position[s[2][0]] if s[2][0] >= 0 else -1
            y = int(s[2][1]) if s[2][0] < 0 else 0
        elif op in ("inc", "dec"):
            c = position[n["cf"][0]] if n["cf"][0] >= 0 else -1
            y = n["cf"][1]
        elif op in ("sext", "cvtsi2ss"):
            x = n["sw"]
        elif op in ("shl", "shr", "sar", "rol", "ror"):
            b, x = -1, n["count"]
        elif op == "bt":
            x, y = n["bit"], {None: 2, 1: 1, 0: 0}[n.get("set")]
        elif op in ("mul", "mul:hi", "imul:lo", "imul:hi", "div", "div:hi", "idiv", "idiv:hi"):
            c = operand(s[2])
        lines.append(" ".join(map(str, (code, w, a, b, c, cond, expect, x, y, *extra))))
    root = []
    tail = len(lines)
    if program.root[0] < 0:
        root = [pool_index[v] for v in root_base]
    else:
        code = OPS.index("mix")
        at = position[-1] + (max(1, (nodes[-1]["w"] + 127) // 128) if nodes[-1]["op"] == "fn" else 1) if nodes else start
        for k, (base, parts) in enumerate(extra_mix):
            pieces = [part(p, i) for i, p in enumerate(parts) if p]
            lines.append(" ".join(map(str, (code, 128, -1, pool_index[base], -1, 0, 0, 0, 0, len(pieces), *pieces))))
            root.append(at + k)
    written = sorted(program.written)
    ranges = []
    for a in written:
        if ranges and ranges[-1][1] == a:
            ranges[-1][1] = a + 1
        else:
            ranges.append([a, a + 1])
    head = ["program", len(pool), len(lines), len(root), len(ranges)]
    assert tail <= len(lines)
    return [" ".join(map(str, head)), " ".join(f"{v:x}" for v in pool), " ".join(map(str, root)),
            " ".join(f"{s:x} {e:x}" for s, e in ranges)] + lines


def export(compiler, vectors, out):
    """vectors: [(overrides, expected f32 bits)]."""
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    domain = Domain(compiler.domain)
    dom = bytearray(domain.size)
    known = bytearray(domain.size)
    with (out / "arena.bin").open("wb") as arena:
        table = []
        at = 0
        for r in compiler.regions:
            if len(r.data) > 1 << 26:
                raise ValueError(f"region too large for the arena: {r.name} {len(r.data)}")
            arena.write(r.data)
            arena.write(r.known)
            table.append((r.start, len(r.data), at))
            at += 2 * len(r.data)
            for s, e in domain.ranges:
                lo, hi = max(s, r.start), min(e, r.start + len(r.data))
                for a in range(lo, hi):
                    k = domain.offset(a)
                    dom[k], known[k] = r.data[a - r.start], r.known[a - r.start]
    lines = [f"regions {len(table)}"] + [f"{s:x} {n} {o}" for s, n, o in table]
    lines += [f"domain {len(domain.ranges)} {domain.size}"] + [f"{s:x} {e:x}" for s, e in domain.ranges]
    lines += [dom.hex(), known.hex()]
    lines.append(f"tables {len(compiler.subs)}")
    for programs in compiler.subs:
        lines.append(f"table {len(programs)}")
        for q in programs:
            q.root_bytes = 12 + len(q.outs)
            lines += lower(q, domain)
    lines.append(f"mains {len(compiler.programs)}")
    for q in compiler.programs:
        q.root_bytes = 4
        lines += lower(q, domain)
    (out / "programs.txt").write_text("\n".join(lines) + "\n", encoding="ascii", newline="\n")
    rows = []
    for overrides, bits in vectors:
        bytes_ = compiler.bytewise(overrides)
        rows.append(f"{bits:08x} " + " ".join(f"{domain.offset(a)}:{b}" for a, b in sorted(bytes_.items())))
    (out / "vectors.txt").write_text("\n".join(rows) + "\n", encoding="ascii", newline="\n")
    mapped = bytearray(domain.size)
    for s, e in domain.ranges:
        for a in range(s, e):
            mapped[domain.offset(a)] = compiler.ev._template_byte(a) is not None
    (out / "mapped.bin").write_bytes(mapped)
    print("exported", len(compiler.programs), "mains", sum(len(x) for x in compiler.subs), "callee paths", "arena MB", at / 1e6, "domain", domain.size)
