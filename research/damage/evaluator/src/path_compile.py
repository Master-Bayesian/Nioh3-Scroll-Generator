"""Compile one executed path of native MAIN into a small guarded program, so a candidate costs a table walk, not a VM run.

The VM stays the source of truth. A trace runs the normal replay and shadows every instruction at byte level: a byte is
tainted only if it can come from the declared input domain (the words a candidate may write). Anything outside the domain
is identical for every candidate, so it folds to a constant. Each tainted operation becomes a node holding its operands.
Every branch, conditional move, jump table and state-indexed store that depends on input becomes a guard, so a program
either reproduces the VM on the same path or reports that it left the path, and the caller replays with the VM.

A candidate writing outside the domain is refused rather than approximated.
"""
import bisect
import math
import re
import struct

MASK64 = (1 << 64) - 1
GP = ("rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp") + tuple(f"r{i}" for i in range(8, 16))
CONST = object()  # a stored byte that holds a path constant, even inside the input domain


CALLEE_SAVED = ("rbx", "rbp", "rsi", "rdi", "r12", "r13", "r14", "r15") + tuple(f"xmm{i}" for i in range(6, 16))
VOLATILE = ("rax", "rcx", "rdx", "r8", "r9", "r10", "r11") + tuple(f"xmm{i}" for i in range(6))


class LeftPath(Exception):
    """The candidate does not follow this program's path."""


class NotPure(Exception):
    """A function chosen for outlining touched caller state beyond its registers."""


def fbits(u):
    return struct.unpack("<f", struct.pack("<I", u & 0xFFFFFFFF))[0]


def bitsf(x):
    return struct.unpack("<I", struct.pack("<f", x))[0]


def signed(v, w):
    return v - (1 << w) if v >> (w - 1) & 1 else v


class Trace:
    def __init__(self, domain, regs):
        self.domain, self.REGS = domain, regs  # sorted [(start, end)]
        self.starts = [a for a, _ in domain]
        self.nodes = []
        self.reg = {}
        self.mem = {}
        self.leaves = {}
        self.flags = None
        self.written = set()
        self.pc = 0
        self.parent = None

    def new(self, op, s=(), w=32, v=0, **meta):
        self.nodes.append({"op": op, "s": [list(x) for x in s], "w": w, "v": v, "rva": self.pc, **meta})
        return len(self.nodes) - 1

    def in_domain(self, a):
        k = bisect.bisect_right(self.starts, a) - 1
        return k >= 0 and a < self.domain[k][1]

    def unify(self, parts, value):
        """One node for a byte list; a mix node when the bytes come from several places."""
        if not any(parts):
            return None
        first = parts[0]
        if first and all(p and p[0] == first[0] and p[1] == i for i, p in enumerate(parts)) and self.nodes[first[0]]["w"] == 8 * len(parts):
            return first[0]
        mask = 0
        for i, p in enumerate(parts):
            if p is None:
                mask |= 0xFF << (8 * i)
        return self.new("mix", (), 8 * len(parts), value, parts=[list(p) if p else None for p in parts], base=value & mask)

    def reg_bytes(self, name):
        full, w, o = self.REGS[name]
        cells = self.reg.get(full) or [None] * (16 if full.startswith("xmm") else 8)
        return cells[o // 8:(o + w) // 8]

    def reg_store(self, name, parts):
        full, w, o = self.REGS[name]
        cells = list(self.reg.get(full) or [None] * (16 if full.startswith("xmm") else 8))
        cells[o // 8:o // 8 + len(parts)] = parts
        if w == 32 and not full.startswith("xmm"):
            cells[4:8] = [None] * 4
        self.reg[full] = cells

    def cell(self, address):
        c = self.mem.get(address)
        up = self.parent
        while c is None and up is not None:
            c = up.mem.get(address)
            if c is not None and c is not CONST:
                raise NotPure(f"reads tainted caller memory {address:#x}")
            up = up.parent
        return c

    def mem_bytes(self, address, size):
        cells = [self.cell(a) for a in range(address, address + size)]
        fresh = [c is None and self.in_domain(address + i) for i, c in enumerate(cells)]
        if all(fresh):
            key = (address, size)
            if key not in self.leaves:
                self.leaves[key] = self.new("in", (), size * 8, 0, address=address)
            return [(self.leaves[key], i) for i in range(size)]
        out = []
        for i, c in enumerate(cells):
            if fresh[i]:
                key = (address + i, 1)
                if key not in self.leaves:
                    self.leaves[key] = self.new("in", (), 8, 0, address=address + i)
                out.append((self.leaves[key], 0))
            else:
                out.append(None if c is None or c is CONST else c)
        return out

    def mem_store(self, address, parts):
        if self.parent is not None and not self.entry_sp - 0x10000 <= address < self.entry_sp + 0x1000:
            raise NotPure(f"store outside the frame {address:#x}")
        for i, p in enumerate(parts):
            self.mem[address + i] = p if p else CONST
            self.written.add(address + i)


def peek(vm, address, size):
    try:
        r, o = vm.mem._at(address, size)
    except Exception:
        return 0
    return int.from_bytes(r.data[o:o + size], "little")


def _slot(node, value):
    return (-1 if node is None else node, value)


def install(state, replay, compiler):
    """Shadow native_vm.VM.step; state[0] is the active Trace, or None when not tracing."""
    import native_vm
    from native_vm import ALIASES, REGS
    base_step = native_vm.VM.step

    def step(vm):
        t = state[0]
        if t is None:
            return base_step(vm)
        t.pc = vm.pc
        row = vm.code[vm.pc]
        m = ALIASES.get(row["mnemonic"], row["mnemonic"])
        ops = [s.strip() for s in row["operands"].split(",")] if row["operands"].strip() else []

        def width(op, fallback=64):
            try:
                return vm.width(op, fallback)
            except Exception:
                return fallback

        def address(op):
            """(tainted address node or None, concrete address)."""
            concrete = vm.address(op)
            body = op[op.index("[") + 1:op.rindex("]")].replace(" ", "")
            terms = []
            for term in re.findall(r"[+-]?[^+-]+", body):
                sign = -1 if term.startswith("-") else 1
                term = term.lstrip("+-")
                factor, name = sign, None
                for piece in term.split("*"):
                    if piece in REGS:
                        name = piece
                    elif piece != "rip":
                        factor *= int(piece, 0)
                    else:
                        factor = None
                if name and factor is not None:
                    node = t.unify(t.reg_bytes(name), vm.regget(name).value)
                    if node is not None:
                        terms.append((node, factor))
            if not terms:
                return None, concrete
            rest = concrete
            for node, factor in terms:
                rest -= factor * t.nodes[node]["v"]
            return t.new("addr", [(n, 0) for n, _ in terms], 64, concrete, factors=[f for _, f in terms], disp=rest & MASK64), concrete

        def guard_equal(node):
            t.new("eq", [(node, 0)], 1, 1, expect=t.nodes[node]["v"])

        def read_bytes(op, size):
            """Byte shadow of a register or memory operand, the load made explicit when its address depends on input."""
            if op in REGS:
                return t.reg_bytes(op)[:size]
            if "[" not in op:
                return [None] * size
            node, at = address(op)
            if node is None:
                return t.mem_bytes(at, size)
            if any(a in t.written for a in range(at, at + size)):
                guard_equal(node)
                return t.mem_bytes(at, size)
            load = t.new("load", [(node, 0)], size * 8, peek(vm, at, size), size=size)
            return [(load, i) for i in range(size)]

        def concrete(op, w):
            if op in REGS:
                v = vm.regget(op)
                return (v.value if w is None or w >= v.width else v.view(w).value)
            if "[" in op:
                return peek(vm, vm.address(op), width(op, w) // 8)
            return int(op, 0) & ((1 << (w or 64)) - 1)

        def src(op, w):
            value = concrete(op, w)
            return _slot(t.unify(read_bytes(op, w // 8), value), value)

        target = None
        if ops and "[" in ops[0] and m not in ("nop", "lea", "cmp", "test", "comiss", "ucomiss", "bt", "push", "jmp", "call"):
            node, at = address(ops[0])
            if node is not None:
                guard_equal(node)
            target = at

        def write_bytes(parts, op=None):
            op = ops[0] if op is None else op
            if op in REGS:
                t.reg_store(op, parts)
            else:
                t.mem_store(target, parts)

        def write(node, w, op=None):
            write_bytes([(node, i) for i in range(w // 8)] if node is not None else [None] * (w // 8), op)

        def result_value(op, w):
            return vm.regget(op).view(w).value if op in REGS else peek(vm, target, w // 8)

        def finish(name, slots, w, flags=False, dest=True, **meta):
            """After the VM step: a node when an operand is tainted, else a constant."""
            base_step(vm)
            if not any(s[0] >= 0 for s in slots):
                if dest:
                    write(None, w)
                if flags:
                    t.flags = None
                return None
            n = t.new(name, slots, w, result_value(ops[0], w) if dest else 0, **meta)
            if dest:
                write(n, w)
            if flags:
                t.nodes[n]["flags"] = True
                t.flags = n
            return n

        def flag_slot():
            return _slot(t.flags, 0)

        def cf_slot():
            cf = vm.flags["CF"]
            return (t.flags, 0) if t.flags is not None else (-1, int(bool(cf)))

        if m in ("nop", "endbr64", "jmp", "call", "ret", "push", "pop", "lea") or m.startswith("j"):
            if m == "lea":
                node, _ = address(ops[1])
                w = width(ops[0])
                base_step(vm)
                if node is not None and w != 64:
                    node = t.new("mix", (), w, vm.regget(ops[0]).view(w).value, parts=[[node, i] for i in range(w // 8)], base=0)
                write(node, w)
            elif m == "push":
                value = read_bytes(ops[0], 8)
                sp = vm.regget("rsp").value - 8
                base_step(vm)
                t.mem_store(sp, value)
            elif m == "pop":
                sp = vm.regget("rsp").value
                value = t.mem_bytes(sp, 8)
                base_step(vm)
                t.reg_store(ops[0], value)
            elif m == "call":
                sp = vm.regget("rsp").value - 8
                if "branch_target" not in row:
                    node = t.unify(read_bytes(ops[0], 8), concrete(ops[0], 64))
                    if node is not None:
                        guard_equal(node)
                site = vm.pc
                base_step(vm)
                t.mem_store(sp, [None] * 8)
                if vm.pc in compiler.outline and site not in compiler.inline:
                    state[0] = compiler.enter(t, site, sp, vm)
            elif m == "ret":
                sp = vm.regget("rsp").value
                if any(t.mem_bytes(sp, 8)):
                    raise RuntimeError("tainted return address")
                base_step(vm)
                if t.parent is not None and sp == t.entry_sp:
                    state[0] = compiler.leave(t, vm)
            elif m == "jmp":
                if "branch_target" not in row:
                    node = t.unify(read_bytes(ops[0], 8), concrete(ops[0], 64))
                    if node is not None:
                        guard_equal(node)
                base_step(vm)
            elif m in ("nop", "endbr64"):
                base_step(vm)
            else:
                before, flags = vm.pc, t.flags
                base_step(vm)
                if flags is not None:
                    fall = before + len(bytes.fromhex(row["bytes"]))
                    t.new("guard", [(flags, 0)], 1, 1, cond=m, expect=vm.pc != fall, stack=[c["target"] for c in vm.call_stack[-3:]])
        elif m == "mov":
            w = width(ops[0])
            parts = read_bytes(ops[1], w // 8)
            base_step(vm)
            write_bytes(parts)
        elif m == "movzx":
            w, sw = width(ops[0]), vm.width(ops[1])
            parts = read_bytes(ops[1], sw // 8)
            base_step(vm)
            write_bytes(parts + [None] * ((w - sw) // 8))
        elif m in ("movsx", "movsxd"):
            w, sw = width(ops[0]), vm.width(ops[1])
            finish("sext", [src(ops[1], sw)], w, sw=sw)
        elif m in ("movaps", "movups", "movdqa", "movdqu"):
            parts = read_bytes(ops[1], 16)
            base_step(vm)
            write_bytes(parts)
        elif m in ("movss", "movsd", "movd", "movq"):
            size = 4 if m in ("movss", "movd") else 8
            parts = read_bytes(ops[1], size)
            base_step(vm)
            if ops[0] in REGS:
                full = REGS[ops[0]][0]
                if full.startswith("xmm"):
                    keep = m in ("movss", "movsd") and "[" not in ops[1]
                    cells = list(t.reg.get(full) or [None] * 16)
                    cells[:size] = parts
                    if not keep:
                        cells[size:] = [None] * (16 - size)
                    t.reg[full] = cells
                else:
                    t.reg_store(ops[0], parts)
            else:
                t.mem_store(target, parts)
        elif m in ("addss", "subss", "mulss", "divss", "minss", "maxss"):
            finish(m, [src(ops[0], 32), src(ops[1], 32)], 32)
        elif m == "sqrtss":
            finish(m, [src(ops[1], 32)], 32)
        elif m in ("comiss", "ucomiss"):
            finish("comiss", [src(ops[0], 32), src(ops[1], 32)], 1, True, False)
        elif m == "cvtsi2ss":
            sw = width(ops[1], 32)
            finish(m, [src(ops[1], sw)], 32, sw=sw)
        elif m == "cvttss2si":
            finish(m, [src(ops[1], 32)], width(ops[0]))
        elif m == "cvtdq2ps":
            lanes = [src_lane(t, read_bytes(ops[1], 16), concrete(ops[1], 128), i) for i in range(4)]
            base_step(vm)
            out = vm.regget(ops[0]).value
            parts = []
            for i, s in enumerate(lanes):
                if s[0] < 0:
                    parts += [None] * 4
                else:
                    n = t.new("cvtsi2ss", [s], 32, (out >> (32 * i)) & 0xFFFFFFFF, sw=32)
                    parts += [(n, k) for k in range(4)]
            write_bytes(parts)
        elif m in ("cvtss2sd", "cvtsd2ss"):
            sw, w = (32, 64) if m == "cvtss2sd" else (64, 32)
            finish(m, [src(ops[1], sw)], w)
        elif m in ("andps", "orps", "xorps", "andpd", "orpd", "xorpd", "pand", "por", "pxor"):
            if ops[0] == ops[1] and m in ("xorps", "xorpd", "pxor"):
                base_step(vm)
                write(None, 128)
            else:
                finish({"a": "and", "o": "or", "x": "xor", "p": {"pand": "and", "por": "or", "pxor": "xor"}.get(m)}[m[0]],
                       [src(ops[0], 128), src(ops[1], 128)], 128)
        elif m in ("and", "or", "xor", "test"):
            w = width(ops[0], width(ops[1], 64))
            if ops[0] == ops[1] and m == "xor":
                base_step(vm)
                write(None, w)
                t.flags = None
            else:
                finish("and" if m == "test" else m, [src(ops[0], w), src(ops[1], w)], w, True, m != "test")
        elif m in ("cmp", "add", "sub", "adc", "sbb"):
            w = width(ops[0])
            if m == "sub" and ops[0] == ops[1]:
                base_step(vm)
                write(None, w)
                t.flags = None
            else:
                carry = cf_slot() if m in ("adc", "sbb") else (-1, 0)
                finish("sub" if m == "cmp" else m, [src(ops[0], w), src(ops[1], w), carry], w, True, m != "cmp")
        elif m in ("inc", "dec"):
            w = width(ops[0])
            s = [src(ops[0], w)]
            old = t.flags
            n = finish(m, s, w)
            if n is None:
                t.flags = old if old is None else t.new("keep_cf", [(old, 0)], 1, 0, flags=True)
            else:
                t.nodes[n].update(flags=True, cf=[old if old is not None else -1, int(bool(vm.flags["CF"]))])
                t.flags = n
        elif m == "neg":
            finish(m, [src(ops[0], width(ops[0]))], width(ops[0]), True)
        elif m == "not":
            finish(m, [src(ops[0], width(ops[0]))], width(ops[0]))
        elif m in ("shl", "shr", "sar", "rol", "ror"):
            w = width(ops[0])
            s = [src(ops[0], w), src(ops[1], 8) if len(ops) > 1 else (-1, 1)]
            if s[1][0] >= 0:
                guard_equal(s[1][0])
                s[1] = (-1, s[1][1])
            count = s[1][1] & (63 if w == 64 else 31)
            if count == 0:
                base_step(vm)  # flags and destination keep their values
            else:
                finish(m, s, w, True, count=count)
        elif m == "imul" and len(ops) > 1:
            w = width(ops[0])
            pair = (ops[0], ops[1]) if len(ops) == 2 else (ops[1], ops[2])
            finish("imul", [src(pair[0], w), src(pair[1], w)], w, True)
        elif m in ("mul", "imul", "div", "idiv"):
            w = width(ops[0])
            lo, hi = ("rax", "rdx") if w == 64 else ("eax", "edx")
            slots = [src(lo, w), src(hi, w), src(ops[0], w)]
            base_step(vm)
            t.flags = None
            if any(s[0] >= 0 for s in slots):
                q = t.new(m, slots, w, vm.regget(lo).value)
                r = t.new(m + ":hi", slots, w, vm.regget(hi).value)
                write(q, w, lo)
                write(r, w, hi)
            else:
                write(None, w, lo)
                write(None, w, hi)
        elif m in ("cdq", "cqo", "cdqe"):
            if m == "cdqe":
                s = [src("eax", 32)]
                base_step(vm)
                n = t.new("sext", s, 64, vm.regget("rax").value, sw=32) if s[0][0] >= 0 else None
                write(n, 64, "rax")
            else:
                w = 64 if m == "cqo" else 32
                s = [src("rax" if w == 64 else "eax", w)]
                base_step(vm)
                n = t.new("sign", s, w, vm.regget("rdx" if w == 64 else "edx").value) if s[0][0] >= 0 else None
                write(n, w, "rdx" if w == 64 else "edx")
        elif m in ("bt", "bts", "btr"):
            w = width(ops[0])
            a, b = src(ops[0], w), src(ops[1], w)
            if b[0] >= 0:
                guard_equal(b[0])
                b = (-1, b[1])
            finish("bt", [a], w, True, m != "bt", bit=b[1] % w, set={"bt": None, "bts": 1, "btr": 0}[m])
        elif m.startswith("cmov"):
            w = width(ops[0])
            s, d = src(ops[1], w), src(ops[0], w)
            flags = t.flags
            if flags is None:
                take = vm.condition(m)
                parts = read_bytes(ops[1 if take else 0], w // 8)
                base_step(vm)
                write_bytes(parts)
            else:
                base_step(vm)
                n = t.new("cmov", [(flags, 0), s, d], w, vm.regget(ops[0]).view(w).value, cond=m)
                write(n, w)
        elif m.startswith("set"):
            flags = t.flags
            base_step(vm)
            n = t.new("set", [(flags, 0)], 8, peek(vm, target, 1) if target is not None else vm.regget(ops[0]).value, cond=m) if flags is not None else None
            write(n, 8)
        elif m == "xchg":
            a, b = read_bytes(ops[0], width(ops[0]) // 8), read_bytes(ops[1], width(ops[0]) // 8)
            base_step(vm)
            write_bytes(b, ops[0])
            write_bytes(a, ops[1])
        else:
            raise RuntimeError("path_compile: no rule for " + m + " " + row["operands"])

    native_vm.VM.step = step

    for name in ("getter_cut", "inactive_row_multiplier_cut"):
        original = getattr(replay, name)

        def cut(parent, _original=original):
            out = _original(parent)
            t = state[0]
            if t is not None:  # these cuts leave only constants behind
                for r in ("rax", "rcx", "rdx", "r8", "r9", "r10", "r11", "rbx"):
                    t.reg[r] = None
                for i in range(6):
                    t.reg["xmm" + str(i)] = None
                t.flags = None
            return out

        setattr(replay, name, cut)


def src_lane(t, parts, value, i):
    lane = parts[4 * i:4 * i + 4]
    v = (value >> (32 * i)) & 0xFFFFFFFF
    return _slot(t.unify(lane, v), v)


def condition(m, f):
    """x86 condition from a flag tuple (CF, ZF, SF, OF, PF); a None flag leaves the path."""
    c = m.removeprefix("cmov").removeprefix("set")
    if c.startswith("j"):
        c = c[1:]

    def g(i):
        if f[i] is None:
            raise LeftPath("undefined flag")
        return f[i]
    CF, ZF, SF, OF, PF = 0, 1, 2, 3, 4
    if c in ("e", "z"):
        return g(ZF)
    if c in ("ne", "nz"):
        return not g(ZF)
    if c in ("b", "c", "nae"):
        return g(CF)
    if c in ("ae", "nb", "nc"):
        return not g(CF)
    if c in ("be", "na"):
        return g(CF) or g(ZF)
    if c in ("a", "nbe"):
        return not g(CF) and not g(ZF)
    if c == "s":
        return g(SF)
    if c == "ns":
        return not g(SF)
    if c in ("l", "nge"):
        return g(SF) != g(OF)
    if c in ("ge", "nl"):
        return g(SF) == g(OF)
    if c in ("le", "ng"):
        return g(ZF) or g(SF) != g(OF)
    if c in ("g", "nle"):
        return not g(ZF) and g(SF) == g(OF)
    if c in ("p", "pe"):
        return g(PF)
    if c in ("np", "po"):
        return not g(PF)
    raise LeftPath("condition " + m)


def zsp(v, w):
    return v == 0, bool(v >> (w - 1) & 1), (v & 255).bit_count() % 2 == 0


class Program:
    """Guarded straight-line form of one path. run(read) returns the output bits or raises LeftPath."""

    def __init__(self, trace, root, written, subs):
        self.subs = subs
        keep = set()
        stack = [i for i, n in enumerate(trace.nodes) if n["op"] in ("guard", "eq")]
        if root[0] >= 0:
            stack.append(root[0])
        while stack:
            i = stack.pop()
            if i in keep:
                continue
            keep.add(i)
            n = trace.nodes[i]
            stack.extend(s[0] for s in n["s"] if s[0] >= 0)
            stack.extend(p[0] for p in n.get("parts") or () if p)
            if n.get("cf") and n["cf"][0] >= 0:
                stack.append(n["cf"][0])
        order = sorted(keep)
        at = {old: new for new, old in enumerate(order)}
        self.nodes = []
        for old in order:
            n = dict(trace.nodes[old])
            n["s"] = [[at[s[0]], 0] if s[0] >= 0 else [-1, s[1]] for s in n["s"]]
            if n.get("parts"):
                n["parts"] = [[at[p[0]], p[1]] if p else None for p in n["parts"]]
            if n.get("cf"):
                n["cf"] = [at[n["cf"][0]] if n["cf"][0] >= 0 else -1, n["cf"][1]]
            self.nodes.append(n)
        self.root = [at[root[0]] if root[0] >= 0 else -1, root[1]]
        self.written = written
        self.size = len(self.nodes)
        self.signature = repr([{k: v for k, v in n.items() if k != "v"} for n in self.nodes] + [self.root])

    def run(self, read, args=()):
        """read(address, size) -> unsigned value of the candidate's memory; args are the registers of an outlined call."""
        nodes = self.nodes
        val = [0] * len(nodes)
        fl = [None] * len(nodes)
        for i, n in enumerate(nodes):
            op, s = n["op"], n["s"]
            a = (val[s[0][0]] if s[0][0] >= 0 else s[0][1]) if s else 0
            b = (val[s[1][0]] if s[1][0] >= 0 else s[1][1]) if len(s) > 1 else 0
            w = n["w"]
            if op == "in":
                val[i] = read(n["address"], w // 8)
            elif op == "mulss" or op == "addss" or op == "subss" or op == "divss":
                x, y = fbits(a), fbits(b)
                if not (math.isfinite(x) and math.isfinite(y)):
                    raise LeftPath("non-finite")
                if op == "divss" and y == 0:
                    raise LeftPath("divide by zero")
                r = x * y if op == "mulss" else x + y if op == "addss" else x - y if op == "subss" else x / y
                try:
                    val[i] = bitsf(r)
                except OverflowError:
                    raise LeftPath("overflow")
            elif op == "arg":
                val[i] = args[n["index"]]
            elif op == "fn":
                given = [val[x[0]] if x[0] >= 0 else x[1] for x in s]
                for sub in self.subs[n["table"]]:
                    try:
                        val[i] = sub.run(read, given)
                        break
                    except LeftPath:
                        pass
                else:
                    raise LeftPath(f"call {n['rva']:#x}")
            elif op == "guard" or op == "set" or op == "cmov":
                f = fl[s[0][0]]
                if f is None:
                    raise LeftPath("undefined flags")
                c = condition(n["cond"], f)
                if op == "guard":
                    if c != n["expect"]:
                        raise LeftPath(f"guard {n['rva']:#x} {n['cond']} in {n.get('stack')}")
                elif op == "set":
                    val[i] = int(c)
                else:
                    val[i] = (b if c else (val[s[2][0]] if s[2][0] >= 0 else s[2][1])) & ((1 << w) - 1)
            elif op == "eq":
                if a != n["expect"]:
                    raise LeftPath(f"eq {n['rva']:#x} {a:#x} vs {n['expect']:#x}")
            elif op == "mix":
                r = n["base"]
                for k, p in enumerate(n["parts"]):
                    if p:
                        r |= ((val[p[0]] >> (8 * p[1])) & 0xFF) << (8 * k)
                val[i] = r
            elif op == "addr":
                r = n["disp"]
                for k, f in enumerate(n["factors"]):
                    r += f * val[s[k][0]]
                val[i] = r & MASK64
            elif op == "load":
                size = n["size"]
                if any(x in self.written for x in range(a, a + size)):
                    raise LeftPath("load overlaps a path store")
                v = read(a, size)
                if v is None:
                    raise LeftPath("unmapped")
                val[i] = v
            elif op == "comiss":
                x, y = fbits(a), fbits(b)
                if not (math.isfinite(x) and math.isfinite(y)):
                    raise LeftPath("non-finite")
                fl[i] = (x < y, x == y, False, False, False)
            elif op == "minss" or op == "maxss":
                x, y = fbits(a), fbits(b)
                if not (math.isfinite(x) and math.isfinite(y)):
                    raise LeftPath("non-finite")
                val[i] = (a if x < y else b) if op == "minss" else (a if x > y else b)
            elif op == "sqrtss":
                x = fbits(a)
                if not math.isfinite(x) or x < 0:
                    raise LeftPath("sqrt")
                val[i] = bitsf(math.sqrt(x))
            elif op == "cvtsi2ss":
                v = signed(a, n["sw"])
                if abs(v) >= 1 << 53:
                    raise LeftPath("integer range")
                val[i] = bitsf(float(v))
            elif op == "cvttss2si":
                x = fbits(a)
                if not math.isfinite(x) or abs(x) >= 2.0 ** (w - 1):
                    raise LeftPath("conversion range")
                val[i] = int(x) & ((1 << w) - 1)
            elif op == "cvtss2sd":
                val[i] = struct.unpack("<Q", struct.pack("<d", fbits(a)))[0]
            elif op == "cvtsd2ss":
                val[i] = bitsf(struct.unpack("<d", struct.pack("<Q", a))[0])
            elif op == "and" or op == "or" or op == "xor":
                b &= (1 << w) - 1
                v = a & b if op == "and" else a | b if op == "or" else a ^ b
                val[i] = v
                if n.get("flags"):
                    fl[i] = (False, *zsp(v, w)[:2], False, zsp(v, w)[2])
            elif op == "add" or op == "adc" or op == "sub" or op == "sbb":
                mask = (1 << w) - 1
                carry = 0
                if op in ("adc", "sbb"):
                    c = s[2]
                    carry = int(c[1]) if c[0] < 0 else int(fl[c[0]][0])
                x, y = a & mask, b & mask
                if op in ("add", "adc"):
                    raw = x + y + carry
                    v = raw & mask
                    cf, of = raw > mask, bool((~(x ^ y) & (x ^ v)) >> (w - 1) & 1)
                else:
                    v = (x - y - carry) & mask
                    cf, of = x < y + carry, bool(((x ^ y) & (x ^ v)) >> (w - 1) & 1)
                val[i] = v
                z, sn, p = zsp(v, w)
                fl[i] = (cf, z, sn, of, p)
            elif op == "inc" or op == "dec" or op == "neg":
                mask = (1 << w) - 1
                x = a & mask
                if op == "neg":
                    v = (-x) & mask
                    cf, of = x != 0, bool((x & v) >> (w - 1) & 1)
                else:
                    v = (x + 1 if op == "inc" else x - 1) & mask
                    y = 1
                    c = n["cf"]
                    cf = bool(c[1]) if c[0] < 0 else (fl[c[0]][0] if fl[c[0]] else None)
                    of = bool((~(x ^ y) & (x ^ v)) >> (w - 1) & 1) if op == "inc" else bool(((x ^ y) & (x ^ v)) >> (w - 1) & 1)
                val[i] = v
                z, sn, p = zsp(v, w)
                fl[i] = (cf, z, sn, of, p)
            elif op == "keep_cf":
                f = fl[s[0][0]]
                fl[i] = (f[0] if f else None, None, None, None, None)
            elif op == "not":
                val[i] = a ^ ((1 << w) - 1)
            elif op == "sext":
                val[i] = signed(a, n["sw"]) & ((1 << w) - 1)
            elif op == "sign":
                val[i] = ((1 << w) - 1) if signed(a, w) < 0 else 0
            elif op == "shl" or op == "shr" or op == "sar":
                count, mask = n["count"], (1 << w) - 1
                x = a & mask
                if op == "shl":
                    v, cf = (x << count) & mask, bool(x & (1 << (w - count)))
                else:
                    v, cf = ((signed(x, w) if op == "sar" else x) >> count) & mask, bool(x & (1 << (count - 1)))
                val[i] = v
                z, sn, p = zsp(v, w)
                of = None
                if count == 1:
                    of = bool(v >> (w - 1) & 1) != cf if op == "shl" else bool(x >> (w - 1)) if op == "shr" else False
                fl[i] = (cf, z, sn, of, p)
            elif op == "rol" or op == "ror":
                count, mask = n["count"] % w, (1 << w) - 1
                x = a & mask
                v = ((x << count) | (x >> (w - count))) & mask if op == "rol" else ((x >> count) | (x << (w - count))) & mask
                val[i] = v
                fl[i] = (bool(v & 1) if op == "rol" else bool(v >> (w - 1) & 1), None, None, None, None)
            elif op == "imul":
                prod = signed(a & ((1 << w) - 1), w) * signed(b & ((1 << w) - 1), w)
                v = prod & ((1 << w) - 1)
                over = prod != signed(v, w)
                val[i] = v
                fl[i] = (over, None, None, over, None)
            elif op == "mul" or op == "mul:hi" or op == "imul:hi":
                third = val[s[2][0]] if s[2][0] >= 0 else s[2][1]
                prod = (signed(a, w) * signed(third, w)) if op == "imul:hi" else a * third
                val[i] = (prod >> w if op.endswith(":hi") else prod) & ((1 << w) - 1)
            elif op == "div" or op == "div:hi" or op == "idiv" or op == "idiv:hi":
                third = val[s[2][0]] if s[2][0] >= 0 else s[2][1]
                top = (b << w) | a
                if op.startswith("idiv"):
                    top, third = signed(top, 2 * w), signed(third, w)
                if third == 0:
                    raise LeftPath("integer divide by zero")
                q = abs(top) // abs(third) * (1 if (top < 0) == (third < 0) else -1)
                r = top - q * third
                if (op.startswith("idiv") and not -(1 << (w - 1)) <= q < (1 << (w - 1))) or (not op.startswith("idiv") and not 0 <= q < (1 << w)):
                    raise LeftPath("division overflow")
                val[i] = (r if op.endswith(":hi") else q) & ((1 << w) - 1)
            elif op == "bt":
                bit = n["bit"]
                fl[i] = (bool(a >> bit & 1), None, None, None, None)
                v = a
                if n.get("set") == 1:
                    v = a | (1 << bit)
                elif n.get("set") == 0:
                    v = a & ~(1 << bit)
                val[i] = v
            else:
                raise RuntimeError("no evaluator for " + op)
            if "check" in n and val[i] != n["check"]:
                raise AssertionError((i, n))
        r = self.root
        return val[r[0]] if r[0] >= 0 else r[1]


class Compiler:
    """Holds compiled paths for one evaluator and input domain; falls back to the VM and learns the new path."""

    def __init__(self, ev, domain, outline=()):
        import native_vm
        self.ev, self.REGS = ev, native_vm.REGS
        self.domain = sorted(domain)
        self.programs = []
        self.outline, self.inline = set(outline), set()
        self.subs, self.sub_keys = [], {}
        self.state = [None]
        self.stats = {"compiled": 0, "left": 0, "traces": 0}
        self.reasons = {}
        install(self.state, ev.replay, self)
        scene = ev.template["scene"]
        self.output = scene["main_address_u64"] + 0x218
        self._memory()

    def _memory(self):
        """The candidate-independent memory: the same region compiler the VM starts from, on the bare template."""
        import main_forward as M
        _, memory, _, _ = M.setup(self.ev.template["scene"])[:4]
        self.regions = memory.regions
        self.region_starts = [r.start for r in memory.regions]

    def in_domain(self, a):
        return any(s <= a < e for s, e in self.domain)

    def enter(self, t, site, sp, vm):
        """Start a child trace for a pure callee: its only inputs are the tainted registers and the input domain."""
        child = Trace(self.domain, self.REGS)
        child.parent, child.entry_sp, child.site = t, sp, site
        child.slots, names = [], []
        for full, cells in t.reg.items():
            if cells and any(cells):
                node = t.unify(cells, vm.regget(full).value)
                a = child.new("arg", (), len(cells) * 8, t.nodes[node]["v"], index=len(child.slots))
                child.reg[full] = [(a, i) for i in range(len(cells))]
                child.slots.append((node, 0))
                names.append(full)
        up = t
        while up is not None:  # tainted bytes the callee can reach through a pointer into its caller's frame
            for a, c in up.mem.items():
                if sp <= a < sp + 0x200 and c is not CONST and a not in child.mem:
                    b = child.new("arg", (), 8, (t.nodes[c[0]]["v"] >> (8 * c[1])) & 0xFF if up is t else 0, index=len(child.slots))
                    node = t.new("mix", (), 8, 0, parts=[list(c)], base=0) if up is t else None
                    if node is None:
                        raise NotPure("tainted grandparent frame")
                    child.mem[a] = (b, 0)
                    child.slots.append((node, 0))
                    names.append(a)
            up = up.parent
        child.names = tuple(sorted(map(str, names)))
        child.context = self.context(t, sp, vm)
        child.saved = {r: list(child.reg[r]) if r in child.reg else None for r in CALLEE_SAVED}
        return child

    def context(self, t, sp, vm):
        """Everything constant a callee may depend on: untainted registers and the live constant memory of its callers.

        A program is shared only between calls with the same context, so a constant folded into it is valid wherever it runs."""
        seen = []
        for r in GP + tuple(f"xmm{i}" for i in range(16)):
            if r in ("rax", "r10", "r11", "xmm4", "xmm5") or any(t.reg.get(r) or ()):
                continue
            seen.append((r, vm.regget(r).value))
        cells = {}
        up = t
        while up is not None:
            for a, c in up.mem.items():
                if a >= sp and a not in cells:
                    cells[a] = c
            up = up.parent
        seen.extend((a, peek(vm, a, 1)) for a, c in sorted(cells.items()) if c is CONST)
        return hash(tuple(seen))

    def leave(self, child, vm):
        """Finish a callee: its program joins the table of this call site; the caller sees only RAX and XMM0."""
        for r in CALLEE_SAVED:
            now = child.reg.get(r)
            if (list(now) if now and any(now) else None) != child.saved[r]:
                raise NotPure("callee-saved register " + r)
        t, bits = child.parent, vm.regget("rax").value | vm.regget("xmm0").view(32).value << 64
        cells = (child.reg.get("rax") or [None] * 8) + (child.reg.get("xmm0") or [None] * 16)[:4]
        outs = tuple(sorted(a for a in child.written if a >= child.entry_sp + 8))  # results returned through caller stack slots
        for i, a in enumerate(outs):
            c = child.mem[a]
            cells.append(None if c is CONST else c)
            bits |= peek(vm, a, 1) << (8 * (12 + i))
        root = child.unify(cells, bits)
        key = (child.site, child.names, child.context)
        table = self.sub_keys.setdefault(key, len(self.subs))
        if table == len(self.subs):
            self.subs.append([])
        program = Program(child, (root if root is not None else -1, bits), child.written, self.subs)
        program.outs = outs
        if any(q.outs != outs for q in self.subs[table]):
            raise NotPure("paths of one call site return through different slots")
        if all(q.signature != program.signature for q in self.subs[table]):
            self.subs[table].append(program)
        t.pc = child.site
        n = t.new("fn", child.slots, 96 + 8 * len(outs), bits, table=table)
        for r in VOLATILE:
            t.reg[r] = None
        t.reg["rax"] = [(n, i) for i in range(8)]
        t.reg["xmm0"] = [(n, 8 + i) for i in range(4)] + [None] * 12
        for i, a in enumerate(outs):
            t.mem[a] = (n, 12 + i)
        t.flags = None
        t.written |= child.written
        return t

    def bytewise(self, overrides):
        out = {}
        for address, data in overrides.items():
            for i, b in enumerate(data):
                if self.ev._template_byte(address + i) is not None:
                    out[address + i] = b
        return out

    def reader(self, writes):
        starts, regions = self.region_starts, self.regions

        def read(address, size):
            k = bisect.bisect_right(starts, address) - 1
            if k < 0:
                raise LeftPath("unmapped")
            r = regions[k]
            o = address - r.start
            if o + size > len(r.data) or r.known[o:o + size].count(255) != size:
                missing = [a for a in range(address, address + size) if a not in writes]
                if o + size > len(r.data) or any(r.known[a - r.start] != 255 for a in missing):
                    raise LeftPath("unknown byte")
            value = bytearray(r.data[o:o + size])
            for i in range(size):
                if address + i in writes:
                    value[i] = writes[address + i]
            return int.from_bytes(value, "little")
        return read

    def trace(self, writes):
        """Record the path of one candidate and return its program plus the VM's own output."""
        ev = self.ev
        while True:
            t = Trace(self.domain, self.REGS)
            self.state[0] = t
            try:
                result = ev.replay._execute_request(ev._request(writes), ev.cache)
                break
            except NotPure as why:
                site = self.state[0].site
                print("not outlining", hex(site), why, flush=True)
                self.inline.add(site)
            finally:
                self.state[0] = None
        self.stats["traces"] += 1
        gate = result["score_acceptance"]
        ok = all(gate[k] for k in ("normal_return", "nonvolatile_restoration", "full_original_score_store", "executed_epilogue_status", "finite_score"))
        bits = int.from_bytes(bytes.fromhex(result["output_record_not_automatically_success"]["value_hex"]), "little")
        if not ok:
            return None, None
        parts = t.mem_bytes(self.output, 4)
        root = t.unify(parts, bits)
        program = Program(t, (root if root is not None else -1, bits), t.written, self.subs)
        got = program.run(self.reader(writes))
        if got != bits:
            raise RuntimeError(f"compiled path disagrees with its own trace: {got:#x} vs {bits:#x}")
        if all(q.signature != program.signature for q in self.programs):
            self.programs.insert(0, program)
        return program, bits

    def evaluate(self, overrides):
        writes = self.bytewise(overrides)
        outside = [a for a in writes if not self.in_domain(a)]
        if outside:
            raise ValueError(f"candidate writes outside the input domain: {outside[0]:#x}")
        read = self.reader(writes)
        for program in self.programs:
            try:
                bits = program.run(read)
                self.stats["compiled"] += 1
                return fbits(bits)
            except LeftPath as why:
                self.reasons[str(why)] = self.reasons.get(str(why), 0) + 1
                self.stats["left"] += 1
        _, bits = self.trace(writes)
        return None if bits is None else fbits(bits)
