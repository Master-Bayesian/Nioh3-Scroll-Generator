"""Static search for the conditions that switch effect fields into the damage formula.

For effect fields that the traced hit does not use (not in its formula even with every effect probed), find the MAIN
code that reads them and the branch that skipped that code:

1. Trace the candidate (formula_trace.trace) to get the executed instructions, executed CALL targets and the
   state each traced comparison read.
2. Disassemble (Capstone, original .text bytes) MAIN, every function it called, and their direct callees, by
   recursive descent. Indirect jumps end a path.
3. A site is an instruction whose memory operand has the field's displacement (any base register except RSP/RIP).
   Displacements are not unique to the D object, so sites are candidates until a flip confirms them.
4. For a site that did not execute, walk the control-flow graph backwards to the executed branches whose untaken
   edge leads to it. Those branches are its gates; their traced comparisons name the state. A site in a function
   that never ran is handled through its executed call sites.
argv: <out_dir> [--outfit spec --inventory dir] [--buff 1.1] [--fields D+0x104,...]
"""
import argparse
import json
import sys
from collections import defaultdict, deque
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from formula_trace import _cone, trace  # noqa: E402
from zone_table import NAMES, effect_names, group_fields, infix, leaf_field  # noqa: E402

TEXT_RVA = 0x1000
MAIN = 0x78CE8C
TERMINATORS = ("ret", "jmp", "int3", "ud2")


GETTER = 0x624518  # STAT -> D object getter (returns D in RAX)
VOLATILE = {"rax", "rcx", "rdx", "r8", "r9", "r10", "r11"}
ARGS = ("rcx", "rdx", "r8", "r9")
_SUB = {}
for _full, _parts in (("rax", "eax ax al ah"), ("rbx", "ebx bx bl bh"), ("rcx", "ecx cx cl ch"), ("rdx", "edx dx dl dh"),
                      ("rsi", "esi si sil"), ("rdi", "edi di dil"), ("rbp", "ebp bp bpl"), ("rsp", "esp sp spl")):
    for _n in _parts.split():
        _SUB[_n] = _full
for _k in range(8, 16):
    for _s in ("d", "w", "b"):
        _SUB[f"r{_k}{_s}"] = f"r{_k}"


def canonical(name):
    return _SUB.get(name, name)


def d_holders(code, functions):
    """May-hold dataflow of the D pointer: {function: {rva: frozenset(registers / 'stack+disp')}} before each insn.

    Sources: RAX after CALL GETTER. Copies: MOV reg<-reg, MOV [rsp+x]<-reg, MOV reg<-[rsp+x]. Any other write kills.
    Other CALLs kill volatile registers and pass argument registers holding D to the callee's entry (interprocedural,
    iterated to a fixed point over the analysed functions)."""
    entry = defaultdict(frozenset)
    result = {}
    for _ in range(4):
        changed = False
        for f in functions:
            body, _ = code.function(f)
            state = {f: entry[f]}
            work = deque([f])
            while work:
                rva = work.popleft()
                i = body.get(rva)
                if i is None:
                    continue
                s = set(state[rva])
                m, ops = i["m"], i["operands"]
                if m == "call":
                    if i["target"] is not None and i["target"] != GETTER:
                        args = frozenset(a for a in ARGS if a in s)
                        if args - entry[i["target"]]:
                            entry[i["target"]] = entry[i["target"]] | args
                            changed = True
                    s -= VOLATILE
                    if i["target"] == GETTER:
                        s.add("rax")
                elif m == "mov" and len(ops) == 2 and ops[0] and ops[1]:
                    dst, src = ops
                    held = (src in s) if isinstance(src, str) else (src[1] == "rsp" and f"stack{src[2]:+d}" in s)
                    if isinstance(dst, str):
                        s.discard(dst)
                        if held:
                            s.add(dst)
                    elif dst[1] == "rsp":
                        key = f"stack{dst[2]:+d}"
                        s.discard(key)
                        if held:
                            s.add(key)
                else:
                    for r in i["written"]:
                        s.discard(r)
                s = frozenset(s)
                for n in i["succ"]:
                    if n not in state or not s <= state[n]:
                        state[n] = (state.get(n, frozenset()) | s)
                        work.append(n)
            result[f] = state
        if not changed:
            break
    return result


class Code:
    def __init__(self, text):
        import capstone
        self.text = text
        self.md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
        self.md.detail = True
        self.cache = {}
        self.functions = {}

    def insn(self, rva):
        if rva not in self.cache:
            at = rva - TEXT_RVA
            if not 0 <= at < len(self.text):
                self.cache[rva] = None
                return None
            i = next(self.md.disasm(self.text[at:at + 16], rva), None)
            if i is None:
                self.cache[rva] = None
                return None
            import capstone.x86 as X
            mems, target = [], None
            for op in i.operands:
                if op.type == X.X86_OP_MEM:
                    mems.append((i.reg_name(op.mem.base) if op.mem.base else None, op.mem.disp))
                elif op.type == X.X86_OP_IMM and (i.mnemonic.startswith("j") or i.mnemonic == "call"):
                    target = op.imm
            regs_ops = [canonical(i.reg_name(op.reg)) if op.type == X.X86_OP_REG else
                        ("mem", i.reg_name(op.mem.base) if op.mem.base else None, op.mem.disp) if op.type == X.X86_OP_MEM else None
                        for op in i.operands]
            try:
                written = sorted({canonical(i.reg_name(r)) for r in i.regs_access()[1]})
            except Exception:
                written = []
            self.cache[rva] = {"rva": rva, "size": i.size, "m": i.mnemonic, "ops": i.op_str, "mems": mems, "target": target,
                               "operands": regs_ops, "written": written}
        return self.cache[rva]

    def function(self, entry, limit=40000):
        """Recursive-descent CFG: {rva: insn with 'succ'}, plus direct call targets."""
        if entry in self.functions:
            return self.functions[entry]
        body, calls, work = {}, set(), [entry]
        while work and len(body) < limit:
            rva = work.pop()
            while rva not in body:
                i = self.insn(rva)
                if i is None:
                    break
                nxt = rva + i["size"]
                i = dict(i)
                m = i["m"]
                if m == "call":
                    if i["target"] is not None:
                        calls.add(i["target"])
                    i["succ"] = [nxt]
                elif m == "jmp":
                    i["succ"] = [i["target"]] if i["target"] is not None else []
                elif m.startswith("j"):
                    i["succ"] = [nxt] + ([i["target"]] if i["target"] is not None else [])
                elif m in TERMINATORS:
                    i["succ"] = []
                else:
                    i["succ"] = [nxt]
                body[rva] = i
                for s in i["succ"][1:]:
                    work.append(s)
                if not i["succ"]:
                    break
                rva = i["succ"][0]
        self.functions[entry] = (body, calls)
        return body, calls


def gates_of(body, site, executed):
    """Executed branches in this function whose untaken edge reaches the unexecuted site."""
    preds = defaultdict(list)
    for rva, i in body.items():
        for s in i["succ"]:
            preds[s].append(rva)
    found, seen, queue = set(), {site}, deque([site])
    while queue:
        q = queue.popleft()
        for p in preds.get(q, []):
            if p in executed:
                if q not in executed:
                    found.add(p)
                continue
            if p not in seen:
                seen.add(p)
                queue.append(p)
    return sorted(found)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("out", type=Path)
    p.add_argument("--outfit", default="")
    p.add_argument("--inventory", default=None)
    p.add_argument("--buff", type=float, default=None)
    p.add_argument("--fields", default="", help="comma list like D+0x104; default: named damage fields not in the formula")
    p.add_argument("--exclude", type=Path, action="append", default=[], help="GATES.json files whose formula fields are known")
    args = p.parse_args()
    from fast_eval import Evaluator
    from regress_fast import BASE, CACHE, IN, load_inventory, pieces
    ev = Evaluator(CACHE, BASE / "capture.json", BASE / "ENTRY_REQUEST.json", BASE / "RETAINED_STATIC_CELLS.json",
                   [IN / "grade_table_cells.json"], buff=args.buff)
    m = ev.model
    plist = pieces(args.outfit, load_inventory(args.inventory)) if args.outfit else []
    sh, root, _, _ = trace(ev, plist)
    fields, classes, _ = group_fields(m)
    names = effect_names(m, NAMES)
    by_field = {(o, off): sorted({n for g in gs for n in names.get(g, [])}) for (o, off), gs in fields.items()}
    known = set()
    for n in _cone(sh, root):
        x = sh.nodes[n]
        if x["op"] == "in" and leaf_field(x["label"]):
            known.add(leaf_field(x["label"]))
    for path in args.exclude:
        for f in json.loads(path.read_bytes())["formula_fields"]:
            o, off = f.split("+")
            known.add((o, int(off, 16)))
    if args.fields:
        targets = [(f.split("+")[0], int(f.split("+")[1], 16)) for f in args.fields.split(",")]
    else:
        targets = sorted(k for k, v in by_field.items() if k[0] == "D" and k not in known
                         and any("伤害" in n or "攻击" in n for n in v))
    import replay  # noqa: F401  (path set up by the evaluator)
    sys.path.insert(0, "F:/Nioh3_r16_build/deliverables/pro-r2-return/nioh3-damage-formula-r2-pro-return-pc202-20261018/project-source/r13-curves")
    import indexed_curve as C
    code = Code(C.program().text)
    executed = sh.executed
    entries = {MAIN} | set(sh.call_targets)
    callees = set()
    for e in sorted(entries):
        callees |= code.function(e)[1]
    functions = sorted(entries | callees)
    for f in functions:
        code.function(f)
    holders = d_holders(code, functions)
    decisions = defaultdict(list)
    for d in sh.decisions:
        inputs = sorted({sh.nodes[i]["label"] for i in _cone(sh, d["compare"]) if sh.nodes[i]["op"] == "in"})
        decisions[int(d["rva"], 16)].append({"inputs": inputs, "compare": infix(sh, d["compare"])[:300], "taken": d["taken"]})
    call_sites = defaultdict(list)  # callee -> executed-function call sites
    for f in entries:
        for rva, i in code.function(f)[0].items():
            if i["m"] == "call" and i["target"] is not None:
                call_sites[i["target"]].append((f, rva))

    def describe_branch(f, rva):
        body = code.function(f)[0]
        i = body[rva]
        prev = [x for x in body.values() if x["succ"] and x["succ"][0] == rva and x["rva"] + x["size"] == rva]
        return {"branch": hex(rva), "instruction": f"{i['m']} {i['ops']}",
                "compare": f"{prev[0]['m']} {prev[0]['ops']}" if prev else None,
                "traced": decisions.get(rva) or decisions.get(prev[0]["rva"] if prev else -1) or []}

    report = []
    for obj, off in targets:
        sites = []
        for f in functions:
            body, _ = code.function(f)
            for rva, i in body.items():
                held = holders.get(f, {}).get(rva, frozenset())
                if any(disp == off and base is not None and canonical(base) in held for base, disp in i["mems"]):
                    sites.append((f, rva, i))
        rows = []
        for f, rva, i in sites:
            row = {"function": hex(f), "site": hex(rva), "instruction": f"{i['m']} {i['ops']}", "executed": rva in executed}
            if not row["executed"]:
                if f in entries:
                    row["gates"] = [describe_branch(f, g) for g in gates_of(code.function(f)[0], rva, executed)]
                else:
                    row["gates"] = [dict(describe_branch(cf, g), via_call=hex(cs))
                                    for cf, cs in call_sites.get(f, [])
                                    for g in gates_of(code.function(cf)[0], cs, executed)]
            rows.append(row)
        report.append({"field": f"{obj}+{off:#x}", "effects": by_field.get((obj, off), [])[:8], "sites": rows})
        print(json.dumps({"field": f"{obj}+{off:#x}", "sites": len(rows),
                          "unexecuted_with_gates": sum(bool(r.get("gates")) for r in rows)}, ensure_ascii=False), flush=True)
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "STATIC_GATES.json").write_text(json.dumps({"functions": len(functions), "fields": report}, indent=1,
                                                           ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps({"functions": len(functions), "fields": len(report)}))


if __name__ == "__main__":
    main()
