"""Readable damage formula of one hit: shadow dataflow over the native MAIN replay.

Runs the same replay as fast_eval.Evaluator and, alongside every original instruction, tracks where each value
came from. Leaves are captured actor/target state bytes (labelled by capture role and offset); module constants,
stack scratch and static tables are constants. Every operation with at least one state-dependent operand becomes a
node holding its concrete result, so the damage at MAIN+0x218 is the root of an exact expression DAG of this
execution path. Comparisons and branches whose operands depend on state are recorded as decisions: they are the
conditions (triggers) that select this path.

The DAG is path-specific: a different branch outcome can change the formula. Decisions list every place where it can.
argv: <out_dir> [--outfit F0off,F0off@slot ...] [--inventory dir] [--buff 1.1]
"""
import argparse
import json
import re
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))

POINTER = 1 << 36  # values at or above this are treated as addresses, not numbers
VOLATILE = ("rax", "rcx", "rdx", "r8", "r9", "r10", "r11")


def f32(u):
    return struct.unpack("<f", struct.pack("<I", u & 0xFFFFFFFF))[0]


class Shadow:
    """Expression nodes and per-location provenance for one replay."""

    def __init__(self, labels, regs, image, stack):
        self.image, self.stack = image, stack
        self.labels = labels  # sorted [(start, end, name)]
        self.REGS = regs
        self.nodes = []  # dicts: op, args, value_u, rva
        self.reg = {}  # full register -> GP: node | XMM: [lane0..lane3] (32-bit lanes)
        self.mem = {}  # byte address -> (node, base address, width)
        self.leaves = {}
        self.flags = None
        self.decisions = []
        self.executed = set()  # every executed instruction RVA
        self.call_targets = set()  # every executed CALL destination RVA
        self.call_sites = {}  # CALL site RVA -> destination RVA (last execution)

    def new(self, op, args, value_u, rva, width=32, **meta):
        self.nodes.append({"op": op, "args": args, "value_u": value_u, "width": width, "rva": rva, **meta})
        return len(self.nodes) - 1

    def label(self, address):
        """Capture role and offset for state memory; None for module image and the ABI stack (constants/scratch)."""
        if self.image[0] <= address < self.image[1] or self.stack[0] <= address < self.stack[1]:
            return None
        lo, hi = 0, len(self.labels)
        while lo < hi:
            mid = (lo + hi) // 2
            if self.labels[mid][0] <= address:
                lo = mid + 1
            else:
                hi = mid
        best = None
        for k in range(lo - 1, max(lo - 60, -1), -1):
            s, e, name = self.labels[k]
            if s <= address < e and (best is None or e - s < best[1] - best[0]):
                best = (s, e, name)
        if best is None and lo:  # outside every captured role: nearest role start below, if close
            s, e, name = max(self.labels[max(lo - 60, 0):lo], key=lambda x: x[0])
            if address - s < 0x10000:
                best = (s, e, name)
        return f"{best[2]}+{address - best[0]:#x}" if best else f"heap:{address:#x}"

    # registers
    def rget(self, name, lanes=1):
        full, width, offset = self.REGS[name]
        sh = self.reg.get(full)
        if full.startswith("xmm"):
            sh = sh or [None] * 4
            return sh[0] if lanes == 1 else tuple(sh[:lanes])
        return sh

    def rset(self, name, node, lanes=None):
        full, width, offset = self.REGS[name]
        if full.startswith("xmm"):
            cur = list(self.reg.get(full) or [None] * 4)
            if lanes == "all":
                cur = list(node)
            elif lanes == "zero_upper":
                cur = [node, None, None, None]
            elif lanes == "two":
                cur = [node, node, cur[2], cur[3]]
            else:
                cur[0] = node
            self.reg[full] = cur
        else:
            self.reg[full] = node

    # memory
    def mget(self, address, size, value_u, rva):
        entries = {self.mem.get(a) for a in range(address, address + size)}
        if entries == {None}:
            name = self.label(address)
            if name is None:
                return None
            key = (address, size)
            if key not in self.leaves:
                self.leaves[key] = self.new("in", [], value_u, rva, size * 8, label=name)
            return self.leaves[key]
        if len(entries) == 1:
            node, base, width = next(iter(entries))
            if base == address and width == size:
                return node
            return self.new("view", [node], value_u, rva, size * 8, offset=address - base)
        return self.new("mix", sorted({e[0] for e in entries if e}), value_u, rva, size * 8)

    def mset(self, address, size, node):
        for a in range(address, address + size):
            self.mem[a] = None if node is None else (node, address, size)
        if node is None:
            for a in range(address, address + size):
                self.mem.pop(a, None)


def peek(vm, address, size):
    """Current bytes without an audited read (the instruction itself performs the real, audited read)."""
    if size > 8:
        return 0
    try:
        r, o = vm.mem._at(address, size)
    except Exception:
        return 0
    return int.from_bytes(r.data[o:o + size], "little")


def install(shadow_ref, replay):
    import native_vm
    from native_vm import REGS
    base_step = native_vm.VM.step

    def operand_get(vm, sh, op, width, rva):
        """Shadow of a source operand, or None for constants."""
        if op in REGS:
            return sh.rget(op, 4 if width == 128 else (2 if width == 64 and op.startswith("xmm") else 1))
        if "[" in op:
            address = vm.address(op)
            size = (vm.width(op, width) or width) // 8
            value = peek(vm, address, size)
            content = sh.mget(address, size, value, rva)
            index = [r for r in re.findall(r"[a-z][a-z0-9]+", op[op.index("["):]) if r in REGS]
            tracked = [sh.rget(r) for r in index if sh.rget(r) is not None and vm.regget(r).value < POINTER]
            if tracked:  # table lookup indexed by a state-dependent value
                return sh.new("lookup", tracked + ([content] if content is not None else []), value, rva, size * 8,
                              table=op, address=address)
            return content
        return None

    def step(vm):
        sh = shadow_ref[0]
        if sh is None:
            return base_step(vm)
        rva = hex(vm.pc)
        sh.executed.add(vm.pc)
        row = vm.code[vm.pc]
        m = native_vm.ALIASES.get(row["mnemonic"], row["mnemonic"])
        ops = [s.strip() for s in row["operands"].split(",")] if row["operands"].strip() else []

        def width(op, fallback=64):
            try:
                return vm.width(op, fallback)
            except Exception:
                return fallback

        def src(i, w=None):
            return operand_get(vm, sh, ops[i], w or width(ops[i]), rva)

        pre = {}
        dst_addr = None
        if ops and "[" in ops[0] and m not in ("lea",):
            dst_addr = (vm.address(ops[0]), width(ops[0], 64) // 8)
        if m in ("mov", "movzx", "movsx", "movsxd", "movaps", "movups", "movdqa", "movdqu", "movss", "movsd", "movd", "movq",
                 "cvtsi2ss", "cvttss2si", "cvtss2si", "cvtss2sd", "cvtsd2ss", "cvtdq2ps", "sqrtss"):
            w = 128 if m in ("movaps", "movups", "movdqa", "movdqu") else 32 if m in ("movss", "movd", "cvttss2si", "sqrtss", "cvtss2sd") else 64 if m in ("movsd", "movq", "cvtsd2ss") else None
            pre["s"] = src(1, w) if len(ops) > 1 else None
        elif m in ("addss", "subss", "mulss", "divss", "minss", "maxss", "add", "sub", "adc", "sbb", "imul", "and", "or", "xor",
                   "andps", "orps", "xorps", "andpd", "orpd", "xorpd", "pand", "por", "pxor", "shl", "shr", "sar", "rol", "ror",
                   "cmp", "test", "comiss", "ucomiss", "comisd", "ucomisd", "bt", "bts", "btr"):
            w = 32 if m.endswith("ss") else 128 if m in ("andps", "orps", "xorps", "andpd", "orpd", "xorpd", "pand", "por", "pxor") else None
            pre["a"] = src(0, w)
            pre["b"] = src(1, w) if len(ops) > 1 else None
            if m == "imul" and len(ops) == 3:
                pre["a"], pre["b"] = src(1), None
            if ops and len(ops) > 1 and ops[0] == ops[1] and m in ("xor", "xorps", "xorpd", "pxor", "sub"):
                pre["a"] = pre["b"] = None
                pre["zero"] = True
        elif m in ("inc", "dec", "neg", "not"):
            pre["a"] = src(0)
        elif m.startswith("cmov"):
            pre["s"], pre["a"] = src(1), src(0)
        elif m in ("mul", "div", "idiv"):
            pre["a"], pre["b"], pre["d"] = sh.rget("rax"), src(0), sh.rget("rdx")
        elif m in ("cdq", "cqo", "cdqe"):
            pre["a"] = sh.rget("rax")
        elif m == "push":
            pre["s"] = src(0, 64)
        elif m == "pop":
            sp = vm.regget("rsp").exact()
            pre["s"] = sh.mget(sp, 8, peek(vm, sp, 8), rva)
        elif m == "xchg":
            pre["a"], pre["b"] = src(0), src(1)
        elif m == "lea":
            regs = [r for r in re.findall(r"[a-z][a-z0-9]+", ops[1]) if r in REGS]
            pre["regs"] = [sh.rget(r) for r in regs if sh.rget(r) is not None]
        flags_before = sh.flags
        old_sp = vm.regget("rsp").value
        base_step(vm)

        def old_get(op, w):
            """Operand value as the instruction saw it: rip-relative operands need the instruction's own pc."""
            saved, vm.pc = vm.pc, int(rva, 16)
            try:
                return vm.get(op, w)
            finally:
                vm.pc = saved

        def result(op_name, args, dest=0):
            args = [a for a in args if a is not None] if isinstance(args, list) else args
            if not args:
                return None
            d = ops[dest]
            if d in REGS:
                v = vm.regget(d)
                value = v.value & 0xFFFFFFFF if REGS[d][0].startswith("xmm") else v.value
            else:
                a, s = dst_addr
                value = peek(vm, a, min(s, 8))
            if not REGS.get(d, ("",))[0].startswith("xmm") and value >= POINTER:
                return None
            return sh.new(op_name, args, value, rva, width(d, 64))

        def put(node, dest=0, lanes=None):
            d = ops[dest]
            if d in REGS:
                sh.rset(d, node, lanes)
            elif dst_addr:
                if isinstance(node, tuple):
                    for i, n in enumerate(node):
                        sh.mset(dst_addr[0] + 4 * i, 4, n)
                else:
                    sh.mset(dst_addr[0], dst_addr[1], node)

        if m in ("mov", "movzx", "movsx", "movsxd", "movd", "movq"):
            node = pre["s"]
            if m in ("movzx", "movsx", "movsxd") and node is not None:
                node = result(m, [node])
            if ops[0] in REGS and not REGS[ops[0]][0].startswith("xmm") and vm.regget(ops[0]).value >= POINTER:
                node = None
            if isinstance(node, tuple):
                node = node[0]
            put(node, lanes="zero_upper" if ops[0].startswith("xmm") else None)
        elif m in ("movaps", "movups", "movdqa", "movdqu"):
            s = pre["s"]
            put(tuple(s) if isinstance(s, tuple) else (s, None, None, None) if s is not None else (None,) * 4, lanes="all")
        elif m in ("movss", "movsd"):
            s = pre["s"][0] if isinstance(pre["s"], tuple) else pre["s"]
            if ops[0].startswith("xmm"):
                put(s, lanes="zero_upper" if "[" in ops[1] else None)
            else:
                put(s)
        elif m in ("cvtsi2ss", "cvttss2si", "cvtss2si", "cvtss2sd", "cvtsd2ss", "cvtdq2ps", "sqrtss"):
            put(result(m, [pre["s"][0] if isinstance(pre["s"], tuple) else pre["s"]]))
        elif m in ("addss", "subss", "mulss", "divss", "minss", "maxss"):
            put(result(m, [pre["a"], pre["b"]] if (pre["a"] is not None or pre["b"] is not None) else []), lanes=None)
            n = sh.nodes[-1] if sh.nodes else None
            if (pre["a"] is None) != (pre["b"] is None) and sh.rget(ops[0]) is not None:
                # record the constant operand so the formula shows it
                const_op = 1 if pre["b"] is None else 0
                raw = old_get(ops[const_op], 32) if const_op == 1 else None
                n["const"] = f32(raw.value) if raw is not None else "dest-before"
                if const_op == 0:
                    n["const_first"] = True
        elif m in ("andps", "orps", "xorps", "andpd", "orpd", "xorpd", "pand", "por", "pxor"):
            if pre.get("zero"):
                put((None,) * 4, lanes="all")
            else:
                a = pre["a"] if isinstance(pre["a"], tuple) else (pre["a"],) * 4
                b = pre["b"] if isinstance(pre["b"], tuple) else (pre["b"],) * 4
                lane0 = result(m, [a[0], b[0]])
                if lane0 is not None:
                    raw = old_get(ops[1], 128) if (a[0] is not None and b[0] is None) else None
                    if raw is not None:
                        sh.nodes[lane0]["mask"] = hex(raw.value & 0xFFFFFFFF)
                put((lane0,) + tuple(x if x is not None else y for x, y in zip(a[1:], b[1:])), lanes="all")
        elif m in ("add", "sub", "adc", "sbb", "imul", "and", "or", "xor", "shl", "shr", "sar", "rol", "ror"):
            if pre.get("zero"):
                put(None)
                sh.flags = None
            else:
                node = result(m, [pre["a"], pre["b"]])
                if node is not None and pre["b"] is None and len(ops) > 1 and ops[1] not in REGS:
                    try:
                        sh.nodes[node]["const"] = int(ops[1], 0) if "[" not in ops[1] else old_get(ops[1], width(ops[0])).value
                    except Exception:
                        pass
                put(node)
                sh.flags = node
        elif m in ("inc", "dec", "neg", "not"):
            node = result(m, [pre["a"]])
            put(node)
            sh.flags = node if m != "not" else sh.flags
        elif m in ("cmp", "test", "comiss", "ucomiss", "comisd", "ucomisd", "bt"):
            if pre["a"] is not None or pre["b"] is not None:
                const = None
                if pre["b"] is None and len(ops) > 1:
                    try:
                        raw = old_get(ops[1], 32 if m.endswith("ss") else width(ops[0]))
                        const = f32(raw.value) if m.endswith("ss") else raw.value
                    except Exception:
                        const = ops[1]
                sh.flags = sh.new("cmp:" + m, [x for x in (pre["a"], pre["b"]) if x is not None], 0, rva, const=const,
                                  operands=row["operands"])
            else:
                sh.flags = None
        elif m.startswith("j") and m != "jmp":
            if flags_before is not None:
                sh.decisions.append({"rva": rva, "cond": m, "taken": vm.pc != int(rva, 16) + len(bytes.fromhex(row["bytes"])),
                                     "compare": flags_before})
        elif m.startswith("set"):
            put(sh.new("set:" + m, [flags_before], vm.regget(ops[0]).value if ops[0] in REGS else 0, rva) if flags_before is not None else None)
            if flags_before is not None:
                sh.decisions.append({"rva": rva, "cond": m, "taken": None, "compare": flags_before})
        elif m.startswith("cmov"):
            if flags_before is not None or pre["s"] is not None or pre["a"] is not None:
                if flags_before is not None:
                    sh.decisions.append({"rva": rva, "cond": m, "taken": None, "compare": flags_before})
                put(result("select:" + m, [flags_before, pre["s"], pre["a"]]))
        elif m in ("mul", "div", "idiv"):
            node = sh.new(m, [x for x in (pre["a"], pre["b"], pre["d"]) if x is not None], vm.regget("rax").value, rva) \
                if any(x is not None for x in (pre["a"], pre["b"], pre["d"])) else None
            sh.rset("rax", node)
            sh.rset("rdx", None if node is None else sh.new(m + ":hi", [node], vm.regget("rdx").value, rva))
        elif m in ("cdq", "cqo"):
            sh.rset("rdx", None if pre["a"] is None else sh.new(m, [pre["a"]], vm.regget("rdx").value, rva))
        elif m == "cdqe":
            sh.rset("rax", None if pre["a"] is None else sh.new(m, [pre["a"]], vm.regget("rax").value, rva))
        elif m == "push":
            sh.mset(old_sp - 8, 8, pre["s"] if not isinstance(pre["s"], tuple) else pre["s"][0])
        elif m == "pop":
            put(pre["s"])
        elif m == "call":
            sh.mset(old_sp - 8, 8, None)
            sh.call_targets.add(vm.pc)
            sh.call_sites[int(rva, 16)] = vm.pc
        elif m == "xchg":
            put(pre["b"], 0)
            put(pre["a"], 1)
        elif m == "lea":
            v = vm.regget(ops[0]).value
            put(sh.new("lea", pre["regs"], v, rva, operands=row["operands"]) if pre["regs"] and v < POINTER else None)
        elif m in ("bts", "btr"):
            put(result(m, [pre["a"], pre["b"]]))
        elif m in ("nop", "endbr64", "ret", "jmp"):
            pass
        else:
            raise RuntimeError("formula_trace: no shadow rule for " + m + " " + row["operands"] + " at " + rva)

    native_vm.VM.step = step

    for name in ("getter_cut", "inactive_row_multiplier_cut"):
        original = getattr(replay, name)

        def cut(parent, _original=original):
            out = _original(parent)
            sh = shadow_ref[0]
            if sh is not None:
                for r in VOLATILE + ("rbx",):
                    sh.reg[r] = None
                for i in range(6):
                    sh.reg["xmm" + str(i)] = None
                sh.flags = None
            return out

        setattr(replay, name, cut)


def render(sh, root, limit=4000):
    """Topologically ordered listing of the root's cone."""
    seen, order = set(), []
    stack = [(root, False)]
    while stack:
        n, done = stack.pop()
        if done:
            order.append(n)
            continue
        if n in seen:
            continue
        seen.add(n)
        stack.append((n, True))
        for a in sh.nodes[n]["args"]:
            if a is not None and a not in seen:
                stack.append((a, False))
    lines = []
    for n in order[-limit:]:
        x = sh.nodes[n]
        v = x["value_u"]
        shown = f"{f32(v):.9g}" if x["width"] in (32, 128) else str(v if v < 1 << 63 else v - (1 << 64))
        if x["op"] == "in":
            lines.append(f"n{n} = IN {x['label']} [{x['width']}b] = {shown} (u={v:#x})")
        else:
            extra = "".join(f" {k}={x[k]}" for k in ("const", "mask", "table", "operands") if k in x)
            lines.append(f"n{n} = {x['op']}({', '.join('n' + str(a) for a in x['args'])}) = {shown} @{x['rva']}{extra}")
    return order, lines


_INSTALLED = {}


def trace(ev, pieces, overrides=None):
    """Shadow-traced replay of one candidate -> (shadow, root node of MAIN+0x218, replay result, model facts).

    overrides: optional {address: bytes} applied after the candidate writes (counterfactual state, e.g. a trigger)."""
    from outfit_model import roles
    from native_vm import REGS
    import demand_fill_loop
    writes, facts = ev.writes(pieces)
    for address, data in (overrides or {}).items():
        for i, b in enumerate(data):
            writes[address + i] = b
    request = ev._request(writes)
    if "ref" not in _INSTALLED:
        _INSTALLED["ref"] = [None]
        install(_INSTALLED["ref"], ev.replay)
    ref = _INSTALLED["ref"]
    labels = sorted((a, a + len(b), role) for role, (a, b, _) in roles(ev.capture).items())
    scene = request["scene"]
    image = (scene["module_base_u64"], scene["module_base_u64"] + demand_fill_loop.IMAGE_SIZE)
    stack = (scene["stack_extent"]["start_u64"], scene["stack_extent"]["start_u64"] + scene["stack_extent"]["bytes"])
    ref[0] = sh = Shadow(labels, REGS, image, stack)
    try:
        result = ev.replay._execute_request(request, ev.cache)
    finally:
        ref[0] = None
    root = sh.mem.get(scene["main_address_u64"] + 0x218, (None,))[0]
    return sh, root, result, facts


def main():
    p = argparse.ArgumentParser()
    p.add_argument("out", type=Path)
    p.add_argument("--outfit", default="")
    p.add_argument("--inventory", default=None)
    p.add_argument("--buff", type=float, default=None)
    args = p.parse_args()
    from fast_eval import Evaluator
    from regress_fast import BASE, CACHE, IN, load_inventory, pieces
    ev = Evaluator(CACHE, BASE / "capture.json", BASE / "ENTRY_REQUEST.json", BASE / "RETAINED_STATIC_CELLS.json",
                   [IN / "grade_table_cells.json"], buff=args.buff)
    p_list = pieces(args.outfit, load_inventory(args.inventory)) if args.outfit else []
    sh, root, result, facts = trace(ev, p_list)
    damage = struct.unpack("<f", bytes.fromhex(result["output_record_not_automatically_success"]["value_hex"]))[0]
    args.out.mkdir(parents=True, exist_ok=True)
    order, lines = render(sh, root) if root is not None else ([], [])
    cone = set(order)
    decisions = []
    for d in sh.decisions:
        c = sh.nodes[d["compare"]]
        _, dl = render(sh, d["compare"], limit=12)
        decisions.append({**d, "compare_expr": dl[-1], "inputs": sorted({sh.nodes[i]["label"] for i in _leaves(sh, d["compare"])}),
                          "feeds_damage": bool(set(_cone(sh, d["compare"])) & cone)})
    (args.out / "FORMULA.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (args.out / "DECISIONS.json").write_text(json.dumps(decisions, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    (args.out / "NODES.json").write_text(json.dumps(sh.nodes) + "\n")
    leaves = sorted({sh.nodes[i]["label"] for i in cone if sh.nodes[i]["op"] == "in"})
    summary = {"damage": damage, "status": result["native"]["status"], "native": {k: result["native"].get(k) for k in ("next_rva", "steps", "dependency")}, "root": root, "cone_nodes": len(order),
               "all_nodes": len(sh.nodes), "leaf_inputs": leaves, "decisions": len(decisions), "facts": facts}
    (args.out / "SUMMARY.json").write_text(json.dumps(summary, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in summary.items() if k != "leaf_inputs"}, ensure_ascii=False))
    print(len(leaves), "leaf inputs")


def _cone(sh, n):
    seen, stack = set(), [n]
    while stack:
        x = stack.pop()
        if x in seen:
            continue
        seen.add(x)
        stack.extend(a for a in sh.nodes[x]["args"] if a is not None)
    return seen


def _leaves(sh, n):
    return [x for x in _cone(sh, n) if sh.nodes[x]["op"] == "in"]


if __name__ == "__main__":
    main()
