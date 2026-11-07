"""Generate a CUDA kernel from an exported path bundle: each program becomes straight C, a failed guard returns false and the next path runs.

Not linked into any product. The kernel must match the bundle's vectors bit for bit before its speed means anything. argv: export_dir out_dir
"""
import bisect
import collections
import os
import shutil
import sys
from pathlib import Path

OPS = ["in", "mulss", "addss", "subss", "divss", "arg", "fn", "guard", "set", "cmov", "eq", "mix", "addr", "load", "comiss", "minss", "maxss",
       "sqrtss", "cvtsi2ss", "cvttss2si", "cvtss2sd", "cvtsd2ss", "and", "or", "xor", "add", "adc", "sub", "sbb", "inc", "dec", "neg", "keep_cf",
       "not", "sext", "sign", "shl", "shr", "sar", "rol", "ror", "imul", "mul", "mul:hi", "imul:lo", "imul:hi", "div", "div:hi", "idiv", "idiv:hi", "bt"]

PRELUDE = r"""#include <cuda.h>
#include <cstdint>
#include <string>
#include <cstdio>
#include <cstdlib>
#include <chrono>
#include <vector>
typedef uint64_t u64; typedef int64_t i64; typedef uint32_t u32; typedef uint16_t u16; typedef uint8_t u8;
#define HD __device__ __forceinline__
struct V { u64 lo, hi; };
struct Fl { u8 cf, zf, sf, of, pf; };
struct Span { u64 s, e, o; };
struct Ctx { const u8* dom; const u8* known; const u8* arena; const Span* domain; int domains; const Span* regions; int region_count; const u16* slim_of; u8* touch; u32* prof; };
#define NOF Fl{2, 2, 2, 2, 2}
HD V mk(u64 a) { return V{a, 0}; }
HD u64 mask(u32 w) { return w >= 64 ? ~0ull : (1ull << w) - 1; }
HD V maskv(V a, u32 w) { if (w >= 128) return a; if (w > 64) { a.hi &= (1ull << (w - 64)) - 1; return a; } return V{a.lo & mask(w), 0}; }
HD float f(V a) { union { u32 u; float x; } c; c.u = (u32)a.lo; return c.x; }
HD V fv(float x) { union { u32 u; float x; } c; c.x = x; return mk(c.u); }
HD bool finite(float x) { union { u32 u; float x; } c; c.x = x; return (c.u & 0x7f800000u) != 0x7f800000u; }
HD i64 sx(u64 v, u32 w) { if (w >= 64) return (i64)v; u32 s = 64 - w; return ((i64)(v << s)) >> s; }
HD u8 par(u64 v) { return (__popc((u32)(v & 255)) & 1) == 0; }
HD Fl zsp(u64 v, u32 w, u8 cf, u8 of) { return Fl{cf, (u8)(v == 0), (u8)((v >> (w - 1)) & 1), of, par(v)}; }
HD bool cond(int code, Fl f, bool& fail) {
    u8 x = 0;
    bool r = false;
    switch (code) {
    case 0: case 1: x = f.zf; r = x != 0; break;
    case 2: case 3: x = f.cf; r = x != 0; break;
    case 4: case 5: x = f.cf | f.zf; r = (f.cf == 1) || (f.zf == 1); if (f.cf == 2 || f.zf == 2) x = 2; else x = 0; break;
    case 6: case 7: x = f.sf; r = x != 0; break;
    case 8: case 9: if (f.sf == 2 || f.of == 2) x = 2; else x = 0; r = f.sf != f.of; break;
    case 10: case 11: if (f.zf == 2 || f.sf == 2 || f.of == 2) x = 2; else x = 0; r = f.zf == 1 || f.sf != f.of; break;
    default: x = f.pf; r = x != 0; break;
    }
    if (x == 2) { fail = true; return false; }
    bool negate = code == 1 || code == 3 || code == 5 || code == 7 || code == 9 || code == 11 || code == 13;
    return negate ? !r : r;
}
HD bool bytes(const u8* p, u32 size, V& out) {
    u64 lo = 0, hi = 0;
    for (u32 i = 0; i < size && i < 8; i++) lo |= (u64)p[i] << (8 * i);
    for (u32 i = 8; i < size; i++) hi |= (u64)p[i] << (8 * (i - 8));
    out = V{lo, hi};
    return true;
}
HD bool all_known(const u8* p, u32 size) { for (u32 i = 0; i < size; i++) if (p[i] != 255) return false; return true; }
HD bool in_(const Ctx& c, u32 at, u32 size, V& out) { if (!all_known(c.known + at, size)) return false; return bytes(c.dom + at, size, out); }
HD bool read_(const Ctx& c, u64 address, u32 size, V& out) {
    for (int i = 0; i < c.domains; i++) {
        Span d = c.domain[i];
        if (address >= d.s && address + size <= d.e) {
            u32 o = (u32)(d.o + address - d.s), w0 = o / 4, w1 = (o + size - 1) / 4;
            if (c.touch) for (u32 w = w0; w <= w1; w++) c.touch[w] = 1;
            u32 s0 = c.slim_of[w0], s1 = c.slim_of[w1];
            if (s0 == 0xffff || s1 == 0xffff || s1 - s0 != w1 - w0) return false;
            return in_(c, s0 * 4 + o % 4, size, out);
        }
    }
    int lo = 0, hi = c.region_count;
    while (lo < hi) { int m = (lo + hi) / 2; if (c.regions[m].s <= address) lo = m + 1; else hi = m; }
    if (lo == 0) return false;
    Span r = c.regions[lo - 1];
    u64 o = address - r.s;
    if (o + size > r.e) return false;
    const u8* base = c.arena + r.o + o;
    if (!all_known(base + r.e, size)) return false;
    return bytes(base, size, out);
}
HD V addv(u64 x, u64 y, u64 carry, u32 w, Fl& fl) {
    x &= mask(w); y &= mask(w);
    u64 raw = x + y + carry;
    u64 v = raw & mask(w);
    bool cf = w == 64 ? (raw < x || (carry && raw == x)) : raw > mask(w);
    u64 of = ((~(x ^ y) & (x ^ v)) >> (w - 1)) & 1;
    fl = zsp(v, w, cf, (u8)of);
    return mk(v);
}
HD V subv(u64 x, u64 y, u64 carry, u32 w, Fl& fl) {
    x &= mask(w); y &= mask(w);
    u64 v = (x - y - carry) & mask(w);
    bool cf = x < y || (carry && x == y);
    u64 of = (((x ^ y) & (x ^ v)) >> (w - 1)) & 1;
    fl = zsp(v, w, cf, (u8)of);
    return mk(v);
}
HD V logic(int kind, V a, V b, u32 w, Fl& fl) {
    b = maskv(b, w);
    V v = kind == 0 ? V{a.lo & b.lo, a.hi & b.hi} : kind == 1 ? V{a.lo | b.lo, a.hi | b.hi} : V{a.lo ^ b.lo, a.hi ^ b.hi};
    u8 top = w > 64 ? (u8)((v.hi >> (w - 65)) & 1) : (u8)((v.lo >> (w - 1)) & 1);
    fl = Fl{0, (u8)((v.lo | v.hi) == 0), top, 0, par(v.lo)};
    return v;
}
HD V shift(int kind, u64 a, u32 n, u32 w, Fl& fl) {
    u64 x = a & mask(w), v;
    bool cf;
    if (kind == 0) { v = n >= 64 ? 0 : (x << n) & mask(w); cf = (w - n) >= 64 ? false : (x >> (w - n)) & 1; }
    else { v = (kind == 2 ? (u64)(sx(x, w) >> n) : x >> n) & mask(w); cf = (x >> (n - 1)) & 1; }
    u8 of = n != 1 ? 2 : kind == 0 ? (u8)((((v >> (w - 1)) & 1) != 0) != cf) : kind == 1 ? (u8)((x >> (w - 1)) & 1) : 0;
    Fl g = zsp(v, w, cf, of);
    fl = g;
    return mk(v);
}
HD V rot(bool left, u64 a, u32 n, u32 w, Fl& fl) {
    u32 k = n % w;
    u64 x = a & mask(w);
    u64 v = k == 0 ? x : left ? ((x << k) | (x >> (w - k))) & mask(w) : ((x >> k) | (x << (w - k))) & mask(w);
    u64 cf = left ? v & 1 : (v >> (w - 1)) & 1;
    fl = NOF; fl.cf = (u8)cf;
    return mk(v);
}
HD V imul2(u64 a, u64 b, u32 w, Fl& fl) {
    i64 x = sx(a & mask(w), w), y = sx(b & mask(w), w);
    i64 p = (i64)((u64)x * (u64)y);
    u64 v = (u64)p & mask(w);
    bool over = w == 64 ? __mul64hi(x, y) != (p >> 63) : p != sx(v, w);
    fl = NOF; fl.cf = over; fl.of = over;
    return mk(v);
}
HD V mulv(int kind, u64 a, u64 b, u32 w) {
    bool hi = kind == 1 || kind == 3, sgn = kind >= 2;
    if (w == 64) {
        if (!hi) return mk(a * b);
        return mk(sgn ? (u64)__mul64hi((i64)a, (i64)b) : __umul64hi(a, b));
    }
    u64 p = sgn ? (u64)(sx(a, w) * sx(b, w)) : a * b;
    return mk((hi ? p >> w : p) & mask(w));
}
HD bool divv(int kind, V a, V b, V d, u32 w, V& out) {
    if ((d.lo | d.hi) == 0 || (w == 64 && (b.lo | b.hi) != 0)) return false;
    bool rem = kind == 1 || kind == 3, sgn = kind >= 2;
    u64 top = w == 64 ? a.lo : (b.lo << w) | a.lo;
    u64 q, r;
    if (!sgn) {
        q = top / d.lo; r = top % d.lo;
        if (q > mask(w)) return false;
    } else {
        if (w == 64 && (i64)top < 0) return false;
        i64 t = w == 32 ? (i64)top : sx(top, 2 * w), m = sx(d.lo, w);
        if (w == 64 && m == -1) return false;
        i64 qq = t / m, rr = t % m;
        i64 limit = w == 64 ? 0x7fffffffffffffffll : (1ll << (w - 1));
        if (w < 64 && (qq < -limit || qq >= limit)) return false;
        q = (u64)qq; r = (u64)rr;
    }
    out = mk((rem ? r : q) & mask(w));
    return true;
}
"""

NL_C = chr(10)
SLIM = {}
NEARBY = 16  # how far ahead a path looks for an operation it can share by choosing values at run time


def word(m, w):
    return m >> (64 * w) & (2**64 - 1)


def lit(v):
    return f"V{{0x{v & (2**64 - 1):x}ull, 0x{v >> 64:x}ull}}"


def parse(path):
    lines = Path(path, "programs.txt").read_text(encoding="ascii").split("\n")
    at = 0

    def take():
        nonlocal at
        at += 1
        return lines[at - 1]

    n = int(take().split()[1])
    regions = [(int(a, 16), int(b), int(c)) for a, b, c in (take().split() for _ in range(n))]
    head = take().split()
    domain, offset = [], 0
    for _ in range(int(head[1])):
        s, e = (int(x, 16) for x in take().split())
        domain.append((s, e, offset))
        offset += e - s
    base, known = bytes.fromhex(take()), bytes.fromhex(take())

    def program():
        h = take().split()
        assert h[0] == "program"
        consts = [int(w, 16) for w in take().split()]
        root = [int(w) for w in take().split()]
        r = take().split()
        written = [(int(r[2 * i], 16), int(r[2 * i + 1], 16)) for i in range(int(h[4]))]
        nodes = [[int(w) for w in take().split()] for _ in range(int(h[2]))]
        return consts, root, written, nodes

    tables = [[program() for _ in range(int(take().split()[1]))] for _ in range(int(take().split()[1]))]
    mains = [program() for _ in range(int(take().split()[1]))]
    return regions, domain, base, known, tables, mains


def inputs(program):
    for n in program[3]:
        if OPS[n[0]] == "in":
            for b in range(n[7], n[7] + n[1] // 8):
                yield b // 4


def slim_words(tables, mains, size, source):
    """The domain words the formula can read: every static input, plus the words loads reached in a full instrumented run."""
    touched = source / "touched.txt"
    if not touched.exists():
        return list(range(size // 4)), True
    words = {int(w) for w in touched.read_text(encoding="ascii").split()}
    for q in mains + [q for programs in tables for q in programs]:
        words.update(inputs(q))
    return sorted(words), False


def walk(program):
    """Each node with the slot it writes."""
    slot = len(program[0])
    for n in program[3]:
        step = n[8] if OPS[n[0]] == "fn" else 1
        yield slot, n
        slot += step


def flag_users(program):
    used = set()
    for _, n in walk(program):
        op = OPS[n[0]]
        if op in ("guard", "set", "cmov", "keep_cf"):
            used.add(n[2])
        elif op in ("adc", "sbb", "inc", "dec") and n[4] >= 0:
            used.add(n[4])
    return used


def demand_pass(program, need, args, roots, tables_args, table_roots):
    """One backward sweep: which bytes of each value can reach anything that matters. Returns False when nothing changed."""
    changed = False

    def want(slot, mask):
        nonlocal changed
        if mask & ~need.get(slot, 0):
            need[slot] = need.get(slot, 0) | mask
            changed = True

    used = flag_users(program)
    for k, s in enumerate(program[1]):
        want(s, roots[k])
    for slot, n in reversed(list(walk(program))):
        op, w, a, b, c, x, y, extra = OPS[n[0]], n[1], n[2], n[3], n[4], n[7], n[8], n[9:]
        here = need.get(slot, 0)
        if op == "fn":
            for k in range(y):
                table_roots[x][k] |= need.get(slot + k, 0)
            for i, s in enumerate(extra):
                want(s, tables_args[x][i])
        elif op == "arg":
            if here & ~args[x]:
                args[x] |= here
                changed = True
        elif op == "mix":
            left = here
            pairs = [extra[q:q + 3] for q in range(1, len(extra), 3)]
            for dst, src, byte in reversed(pairs):
                if left >> dst & 1:
                    want(src, 1 << byte)
                    left &= ~(1 << dst)
            want(b, left)
        elif op in ("mulss", "addss", "subss", "divss"):
            want(a, 0xF); want(b, 0xF)
        elif op in ("minss", "maxss"):
            want(a, 0xF | here); want(b, 0xF | here)
        elif op in ("comiss", "sqrtss", "cvttss2si", "cvtss2sd"):
            want(a, 0xF)
            if op == "comiss":
                want(b, 0xF)
        elif op in ("cvtsd2ss", "cvtsi2ss", "load"):
            want(a, 0xFF)
        elif op == "addr":
            want(b, 0xFF)
            for q in range(1, len(extra), 2):
                want(extra[q], 0xFF)
        elif op in ("and", "or", "xor"):
            mask = here if slot not in used else 0xFFFF
            want(a, mask); want(b, mask)
        elif op == "not":
            want(a, here)
        elif op == "cmov":
            want(b, here); want(c, here)
        elif op == "eq":
            want(a, 0xFFFF); want(b, 0xFFFF)
        elif op in ("div", "div:hi", "idiv", "idiv:hi", "bt"):
            for i in (a, b, c):
                if i >= 0:
                    want(i, 0xFFFF)
        elif op in ("add", "adc", "sub", "sbb", "inc", "dec", "neg", "sext", "sign", "shl", "shr", "sar", "rol", "ror", "imul", "mul", "mul:hi", "imul:lo", "imul:hi"):
            for i in (a, b, c):
                if i >= 0 and not (op in ("adc", "sbb", "inc", "dec") and i == c):
                    want(i, 0xFF)
    return changed


def demands(tables, mains):
    """Byte demand of every value across all programs, so an SSE register rebuilt byte by byte shrinks to the word a float op reads."""
    wide = lambda: collections.defaultdict(int)
    table_args = [wide() for _ in tables]
    table_roots = [wide() for _ in tables]
    needs = {}
    while True:
        changed = False
        for t, ps in enumerate(tables):
            for k, q in enumerate(ps):
                need = needs.setdefault((t, k), {})
                changed = demand_pass(q, need, table_args[t], table_roots[t], table_args, table_roots) or changed
        for k, q in enumerate(mains):
            need = needs.setdefault((-1, k), {})
            changed = demand_pass(q, need, wide(), [0xF] + [0] * 63, table_args, table_roots) or changed
        before = sum(sum(r.values()) for r in table_roots) + sum(sum(a.values()) for a in table_args)
        if not changed and before == demands.seen:
            return needs
        demands.seen = before


demands.seen = -1


def pick_bytes(pairs, base, need, r):
    """A value built only from the bytes something reads, without the byte array."""
    source = {}
    for dst, src, byte in pairs:
        source[dst] = (src, byte)
    terms = {0: [], 1: []}
    for at in range(16):
        if not need >> at & 1:
            continue
        src, byte = source.get(at, (base, at))
        word = f"v{src}.{'lo' if byte < 8 else 'hi'}"
        shift = 8 * (byte % 8)
        terms[at // 8].append((at % 8, word, shift))
    parts = []
    for half in (0, 1):
        runs = []
        for at, word, shift in terms[half]:
            if runs and runs[-1][1] == word and runs[-1][0] + runs[-1][2] == at and runs[-1][3] + 8 * runs[-1][2] == shift:
                runs[-1][2] += 1
            else:
                runs.append([at, word, 1, shift])
        items = []
        for at, word, length, shift in runs:
            value = f"(({word} >> {shift})" + (f" & 0x{(1 << 8 * length) - 1:x}ull)" if length < 8 else ")")
            items.append(f"({value} << {8 * at})")
        parts.append(" | ".join(items) if items else "0ull")
    return f"V {r} = V{{{parts[0]}, {parts[1]}}};"


USES = {"in": "", "arg": "", "fn": "", "guard": "a", "set": "a", "cmov": "abc", "mix": "b", "addr": "b", "load": "a", "keep_cf": "a", "not": "a", "sext": "a",
        "sign": "a", "shl": "a", "shr": "a", "sar": "a", "rol": "a", "ror": "a", "neg": "a", "bt": "a", "sqrtss": "a", "cvtsi2ss": "a", "cvttss2si": "a",
        "cvtss2sd": "a", "cvtsd2ss": "a", "mul": "ac", "mul:hi": "ac", "imul:lo": "ac", "imul:hi": "ac", "div": "abc", "div:hi": "abc", "idiv": "abc",
        "idiv:hi": "abc", "inc": "ac", "dec": "ac", "adc": "abc", "sbb": "abc"}


def arg_fields(n):
    """Positions in a node that name a value."""
    op = OPS[n[0]]
    uses = USES.get(op, "ab")
    fields = [at for field, at in (("a", 2), ("b", 3), ("c", 4)) if field in uses and n[at] >= 0]
    if op == "mix":
        fields += range(11, len(n), 3)
    elif op == "addr":
        fields += range(10, len(n), 2)
    elif op == "fn":
        fields += range(9, len(n))
    return fields


def shape(n):
    """What a node does, without which values it reads. A guard's expected outcome is left out: paths that disagree on it share the test."""
    key = list(n)
    key[2:5] = [-1, -1, -1]
    for at in arg_fields(n):
        key[at] = -2
    if OPS[n[0]] == "guard":
        key[6] = 0
    if OPS[n[0]] == "fn":
        key[7] = -2
    return tuple(key)


def reads(n, name):
    """Which values a node takes; a call also names its table, because two paths may reach one function through different tables."""
    out = {at: name[n[at]] for at in arg_fields(n)}
    if OPS[n[0]] == "fn":
        out[7] = n[7]
    return out


class Entry:
    """One operation of the merged program, and which paths run it."""

    def __init__(self, n, slot, bit, wanted):
        self.node, self.slot, self.mask = n, slot, bit
        self.key = shape(n)
        self.arms = {at: [[bit, s]] for at, s in wanted.items()}
        self.true = bit if n[6] else 0
        self.false = 0 if n[6] else bit


def facts(q, intern):
    """Number a path's values so equal computations in any two paths get equal numbers, and record what its guards establish, each with its position."""
    number = {i: intern(("const", v)) for i, v in enumerate(q[0])}
    tests, equal, made = {}, collections.defaultdict(dict), {}
    for at, (slot, n) in enumerate(walk(q)):
        op = OPS[n[0]]
        key = (shape(n), tuple(number[n[f]] for f in arg_fields(n)), n[7] if op == "fn" else 0)
        for k in range(n[8] if op == "fn" else 1):
            number[slot + k] = intern(key + (k,))
        made[slot] = n
        if op == "guard":
            code, truth = n[5] & ~1, bool(n[6]) ^ bool(n[5] & 1)  # "ne held" and "e failed" are the same fact
            tests.setdefault((number[n[2]], code, truth), at)
            source = made.get(n[2])
            if source and OPS[source[0]] == "sub" and code == 0 and truth and source[3] < len(q[0]):
                equal[number[source[2]]].setdefault(q[0][source[3]], at)
        elif op == "eq":
            equal[number[n[2]]].setdefault(q[0][n[3]], at)
    return number, tests, equal


def exclusive(one, other):
    """Position pairs at which two paths have established opposite things, so at most one of them can still be running."""
    pairs = []
    for (value, code, truth), at in one[1].items():
        there = other[1].get((value, code, not truth))
        if there is not None:
            pairs.append((at, there))
    for value, known in one[2].items():
        for k, at in known.items():
            for j, there in other[2].get(value, {}).items():
                if j != k:
                    pairs.append((at, there))
    return pairs


def merged(programs, needs, origin, tie=None):
    """Paths of one function are mostly the same work, with short differences where a branch went the other way. Line them up into one program: every
    path keeps a bit in `alive`, an operation only some paths need runs while one of them is alive, and a guard clears the bits of the paths it rules
    out, so a failed guard costs one test instead of a restart. A shared operation may read different values per path only when those paths can no
    longer both be alive, because each has passed a test the other failed; otherwise it stays two operations, so every path still computes exactly its
    own trace. `origin` says which table each path belongs to: a call starts with only its table's paths alive, so paths of different tables never run
    together. `tie` lets two paths that cannot be told apart share a call whose only difference is its table; see unite()."""
    consts = list(dict.fromkeys(v for q in programs for v in q[0]))
    index = {v: i for i, v in enumerate(consts)}
    names = {}
    intern = lambda key: names.setdefault(key, len(names))
    known = [facts(q, intern) for q in programs]
    pairs = {}
    value = {i: intern(("const", v)) for i, v in enumerate(consts)}
    need, entries, fresh, roots = {}, [], len(consts), []
    for p, (q, demand) in enumerate(zip(programs, needs)):
        bit = 1 << p
        name = {i: index[v] for i, v in enumerate(q[0])}
        where = collections.defaultdict(list)
        for i, e in enumerate(entries):
            where[e.key].append(i)
        out, cursor = [], 0

        def apart(at, r, there):
            if origin[r] != origin[p]:
                return True
            if (p, r) not in pairs:
                pairs[p, r] = exclusive(known[p], known[r])
            return any(i < at and j < there for i, j in pairs[p, r])

        def alike(field, slot, s):
            return slot == s or (field != 7 and value[slot] == value[s])

        def clear(e, m, at):
            return all(apart(at, r, e.at[r]) for r in range(p) if m >> r & 1)

        def fits(e, wanted, at):
            same_args = all(any(alike(field, slot, s) for _, slot in e.arms[field]) for field, s in wanted.items() if field != 7)
            for field, s in wanted.items():
                for m, slot in e.arms[field]:
                    if alike(field, slot, s) or clear(e, m, at):
                        continue
                    if not (field == 7 and tie and same_args and tie[0](slot, s)):
                        return False
            return True

        def same(e, wanted):
            return all(any(alike(field, slot, s) for _, slot in e.arms[field]) for field, s in wanted.items())

        for at, (old, n) in enumerate(walk(q)):
            key, wanted = shape(n), reads(n, name)
            spots = where.get(key, ())
            spots = spots[bisect.bisect_left(spots, cursor):]
            found = next((i for i in spots if same(entries[i], wanted) and fits(entries[i], wanted, at)), None)
            if found is None:
                found = next((i for i in spots if i < cursor + NEARBY and fits(entries[i], wanted, at)), None)
            step = n[8] if OPS[n[0]] == "fn" else 1
            if found is None:
                e = Entry(n, fresh, bit, wanted)
                e.at = {}
                fresh += step
                out.append(e)
            else:
                e = entries[found]
                out += entries[cursor:found + 1]
                cursor = found + 1
                if tie and 7 in wanted:
                    for m, slot in e.arms[7]:
                        if slot != wanted[7] and not clear(e, m, at):
                            tie[1](slot, wanted[7])
                for field, s in wanted.items():
                    arm = next((a for a in e.arms[field] if a[1] == s), None)
                    if arm:
                        arm[0] |= bit
                    else:
                        e.arms[field].append([bit, s])
                e.mask |= bit
                if OPS[n[0]] == "guard":
                    e.true |= bit if n[6] else 0
                    e.false |= 0 if n[6] else bit
            e.at[p] = at
            for k in range(step):
                name[old + k] = e.slot + k
                value[e.slot + k] = known[p][0][old + k]
        entries = out + entries[cursor:]
        for old, mask in (demand or {}).items():
            if old in name:
                need[name[old]] = need.get(name[old], 0) | mask
        for k, s in enumerate(q[1]):
            if k == len(roots):
                roots.append([])
            arm = next((a for a in roots[k] if a[1] == name[s]), None)
            if arm:
                arm[0] |= bit
            else:
                roots[k].append([bit, name[s]])
    return consts, entries, roots, programs[0][2], need, (1 << len(programs)) - 1


def unite(tables, mains):
    """The compiler keys a callee's table on the constants around the call, which include stale stack bytes, so two paths that nobody can tell apart at a
    call may name different tables although they pass the same values to the same function. Such tables are one table: a caller with those arguments
    gets whichever path of the function its inputs select. Only an identical call by paths with no test between them unites tables, so a function called
    elsewhere with a real constant of its own stays separate."""
    parent = list(range(len(tables)))

    def find(t):
        while parent[t] != t:
            parent[t] = parent[parent[t]]
            t = parent[t]
        return t

    def key(t):
        q = tables[t]
        return max((n[7] for r in q for n in r[3] if OPS[n[0]] == "arg"), default=-1), len(q[0][1]), [tuple(r[2]) for r in q[:1]], [OPS[n[0]] for n in q[0][3][:6]]

    def join(a, b):
        parent[find(b)] = find(a)

    def tied(a, b):
        return key(find(a)) == key(find(b))

    def unique(programs):
        return list({(tuple(q[0]), tuple(q[1]), tuple(q[2]), tuple(map(tuple, q[3]))): q for q in programs}.values())

    while True:
        before = [find(t) for t in range(len(tables))]
        merged(mains, [None] * len(mains), [0] * len(mains), (tied, join))
        roots = [find(t) for t in range(len(tables))]
        if roots == before:
            return tables, mains
        tables = [unique([q for s, ps in enumerate(tables) if roots[s] == t for q in ps]) if roots[t] == t else [] for t in range(len(tables))]
        parent = list(range(len(tables)))
        for q in mains:
            for n in q[3]:
                if OPS[n[0]] == "fn":
                    n[7] = roots[n[7]]
        mains = unique(mains)


def families(tables, needs):
    """Call sites of one function get separate tables whenever the caller's constants differ, so one function can fill fifty tables. Compile the alike
    ones once; a table is then a set of starting bits. Sharing code this way cannot change a result, so judging 'alike' from the opening needs no proof.
    Paths with different stores stay apart: the check on a load depends on what the path wrote."""
    kinds = {}
    for t, programs in enumerate(tables):
        args = max((n[7] for q in programs for n in q[3] if OPS[n[0]] == "arg"), default=-1) + 1
        for k, q in enumerate(programs):
            key = (tuple(map(tuple, q[2])), args, len(q[1]), tuple(OPS[n[0]] for n in q[3][:6]))
            kinds.setdefault(key, []).append((t, k, q))
    groups, calls = [], [[] for _ in tables]
    for f, rows in enumerate(kinds.values()):
        groups.append(merged([q for _, _, q in rows], [needs.get((t, k)) for t, k, _ in rows], [t for t, _, _ in rows]))
        masks = collections.defaultdict(int)
        for i, (t, _, _) in enumerate(rows):
            masks[t] |= 1 << i
        for t, mask in masks.items():
            calls[t].append((f, mask))
    return groups, calls


def emit(name, group, out, known=None):
    consts, entries, roots, written, need, everyone = group
    body = []
    words = range((everyone.bit_length() + 63) // 64)
    start = "".join(f", u64 alive{w}" for w in words) if name.startswith("sub") else ""
    head = f"__noinline__ __device__ bool {name}(const Ctx& c, const V* args, V* out{start})"
    out.append(head + ";")
    code = [head + " {"]
    if written:
        tests = " || ".join(f"(a < 0x{e:x}ull && 0x{s:x}ull < e)" for s, e in written)
        code.insert(0, f"HD bool {name}_w(u64 a, u64 e) {{ return {tests}; }}")
    for i, v in enumerate(consts):
        body.append(f"const V v{i} = {lit(v)};")
    if not start:
        body += [f"u64 alive{w} = 0x{word(everyone, w):x}ull;" for w in words]
    flags = set()
    phis = [max((e.slot + (e.node[8] if OPS[e.node[0]] == "fn" else 1) for e in entries), default=len(consts))]

    def fl(k):
        return f"f{k}" if k in flags else "NOF"

    def test(m):
        return " || ".join(f"(alive{w} & 0x{word(m, w):x}ull)" for w in words if word(m, w))

    def clear(m):
        return " ".join(f"alive{w} &= ~0x{word(m, w):x}ull;" for w in words if word(m, w))

    def line(n, slot, body):
        op, w, a, b, c, cnd, expect, x, y, extra = OPS[n[0]], n[1], n[2], n[3], n[4], n[5], n[6], n[7], n[8], n[9:]
        r, A, B, C = f"v{slot}", f"v{a}", f"v{b}", f"v{c}"
        fw = f"Fl f{slot};"
        step = 1
        if op == "in":
            size, at = w // 8, SLIM[x // 4] * 4 + x % 4
            sure = known is not None and all(v == 255 for v in known[x:x + size])
            low = need.get(slot, 0xFFFF) & ~0xF == 0
            if sure and x % 4 == 0 and (size == 4 or low):
                body.append(f"V {r} = mk(((const u32*)c.dom)[{at // 4}]);")
            elif sure:
                body.append(f"V {r}; bytes(c.dom + {at}, {size}, {r});")
            else:
                body.append(f"V {r}; if (!in_(c, {at}, {size}, {r})) return false;")
        elif op in ("mulss", "addss", "subss", "divss"):
            sign = {"mulss": "*", "addss": "+", "subss": "-", "divss": "/"}[op]
            zero = f" || f({B}) == 0.0f" if op == "divss" else ""
            body.append(f"if (!finite(f({A})) || !finite(f({B})){zero}) return false; float t{slot} = f({A}) {sign} f({B}); if (!finite(t{slot})) return false; V {r} = fv(t{slot});")
        elif op == "arg":
            body.append(f"V {r} = args[{x}];")
        elif op == "fn":
            step = y
            items = ", ".join(f"v{s}" for s in extra)
            body.append(f"V a{slot}[{max(1, len(extra))}] = {{{items}}}; V o{slot}[{y}];")
            if isinstance(x, list):
                body.append(f"bool ok{slot} = false;")
                body.append(" else ".join(f"if ({test(m)}) ok{slot} = table{s}(c, a{slot}, o{slot});" for m, s in x))
                body.append(f"if (!ok{slot}) return false;")
            else:
                body.append(f"if (!table{x}(c, a{slot}, o{slot})) return false;")
            for k in range(y):
                body.append(f"V v{slot + k} = o{slot}[{k}];")
        elif op == "guard":
            body.append(f"V {r} = mk(0); {{ bool bad = false; bool t = cond({cnd}, {fl(a)}, bad); if (bad || t != {'true' if expect else 'false'}) return false; }}")
        elif op == "set":
            body.append(f"V {r}; {{ bool bad = false; {r} = mk(cond({cnd}, {fl(a)}, bad)); if (bad) return false; }}")
        elif op == "cmov":
            body.append(f"V {r}; {{ bool bad = false; bool t = cond({cnd}, {fl(a)}, bad); if (bad) return false; {r} = maskv(t ? {B} : {C}, {w}); }}")
        elif op == "eq":
            body.append(f"V {r} = mk(0); if ({A}.lo != {B}.lo || {A}.hi != {B}.hi) return false;")
        elif op == "mix":
            pairs = [extra[q:q + 3] for q in range(1, len(extra), 3)]
            if slot in need or known is not None:
                body.append(pick_bytes(pairs, b, need.get(slot, 0), r))
                return step
            body.append(f"u8 m{slot}[16]; for (int i = 0; i < 8; i++) {{ m{slot}[i] = (u8)({B}.lo >> (8 * i)); m{slot}[8 + i] = (u8)({B}.hi >> (8 * i)); }}")
            for q in range(1, len(extra), 3):
                dst, src, byte = extra[q:q + 3]
                body.append(f"m{slot}[{dst}] = (u8)({'v%d.lo' % src if byte < 8 else 'v%d.hi' % src} >> {8 * (byte % 8)});")
            body.append(f"V {r} = {{0, 0}}; for (int i = 0; i < 8; i++) {{ {r}.lo |= (u64)m{slot}[i] << (8 * i); {r}.hi |= (u64)m{slot}[8 + i] << (8 * i); }}")
        elif op == "addr":
            terms = "".join(f" + v{extra[q]}.lo * 0x{extra[q + 1] & (2**64 - 1):x}ull" for q in range(1, len(extra), 2))
            body.append(f"V {r} = mk({B}.lo{terms});")
        elif op == "load":
            check = f"if ({name}_w({A}.lo, {A}.lo + {x})) return false; " if written else ""
            body.append(f"V {r}; {check}if (!read_(c, {A}.lo, {x}, {r})) return false;")
        elif op == "comiss":
            flags.add(slot)
            body.append(f"if (!finite(f({A})) || !finite(f({B}))) return false; Fl f{slot} = Fl{{(u8)(f({A}) < f({B})), (u8)(f({A}) == f({B})), 0, 0, 0}}; V {r} = mk(0);")
        elif op in ("minss", "maxss"):
            cmp = "<" if op == "minss" else ">"
            body.append(f"if (!finite(f({A})) || !finite(f({B}))) return false; V {r} = f({A}) {cmp} f({B}) ? {A} : {B};")
        elif op == "sqrtss":
            body.append(f"if (!finite(f({A})) || f({A}) < 0.0f) return false; V {r} = fv(sqrtf(f({A})));")
        elif op == "cvtsi2ss":
            body.append(f"i64 s{slot} = sx({A}.lo, {x}); if (s{slot} >= (1ll << 53) || s{slot} <= -(1ll << 53)) return false; V {r} = fv((float)s{slot});")
        elif op == "cvttss2si":
            body.append(f"if (!finite(f({A})) || fabs((double)f({A})) >= ldexp(1.0, {w - 1})) return false; V {r} = mk((u64)(i64)f({A}) & mask({w}));")
        elif op == "cvtss2sd":
            body.append(f"V {r}; {{ union {{ u64 u; double d; }} q; q.d = (double)f({A}); {r} = mk(q.u); }}")
        elif op == "cvtsd2ss":
            body.append(f"V {r}; {{ union {{ u64 u; double d; }} q; q.u = {A}.lo; {r} = fv((float)q.d); }}")
        elif op in ("and", "or", "xor"):
            flags.add(slot)
            body.append(f"{fw} V {r} = logic({('and', 'or', 'xor').index(op)}, {A}, {B}, {w}, f{slot});")
        elif op in ("add", "adc", "sub", "sbb"):
            flags.add(slot)
            fun = "addv" if op in ("add", "adc") else "subv"
            if op in ("adc", "sbb"):
                if c >= 0:
                    carry = f"(u64){fl(c)}.cf"
                    body.append(f"if ({fl(c)}.cf == 2) return false;")
                else:
                    carry = str(y)
            else:
                carry = "0"
            body.append(f"{fw} V {r} = {fun}({A}.lo, {B}.lo, {carry}, {w}, f{slot});")
        elif op in ("inc", "dec"):
            flags.add(slot)
            cf = f"{fl(c)}.cf" if c >= 0 else str(y)
            sign = "+" if op == "inc" else "-"
            of = f"(~({A}.lo ^ 1) & ({A}.lo ^ t{slot}))" if op == "inc" else f"(({A}.lo ^ 1) & ({A}.lo ^ t{slot}))"
            body.append(f"u64 t{slot} = ({A}.lo {sign} 1) & mask({w}); Fl f{slot} = zsp(t{slot}, {w}, {cf}, (u8)(({of} >> {w - 1}) & 1)); V {r} = mk(t{slot});")
        elif op == "neg":
            flags.add(slot)
            body.append(f"u64 t{slot} = (0 - ({A}.lo & mask({w}))) & mask({w}); Fl f{slot} = zsp(t{slot}, {w}, ({A}.lo & mask({w})) != 0, (u8)((({A}.lo & t{slot}) >> {w - 1}) & 1)); V {r} = mk(t{slot});")
        elif op == "keep_cf":
            flags.add(slot)
            body.append(f"Fl f{slot} = NOF; f{slot}.cf = {fl(a)}.cf; V {r} = mk(0);")
        elif op == "not":
            body.append(f"V {r} = maskv(V{{~{A}.lo, ~{A}.hi}}, {w});")
        elif op == "sext":
            body.append(f"V {r} = mk((u64)sx({A}.lo, {x}) & mask({w}));")
        elif op == "sign":
            body.append(f"V {r} = mk(sx({A}.lo & mask({w}), {w}) < 0 ? mask({w}) : 0);")
        elif op in ("shl", "shr", "sar"):
            flags.add(slot)
            body.append(f"{fw} V {r} = shift({('shl', 'shr', 'sar').index(op)}, {A}.lo, {x}, {w}, f{slot});")
        elif op in ("rol", "ror"):
            flags.add(slot)
            body.append(f"{fw} V {r} = rot({'true' if op == 'rol' else 'false'}, {A}.lo, {x}, {w}, f{slot});")
        elif op == "imul":
            flags.add(slot)
            body.append(f"{fw} V {r} = imul2({A}.lo, {B}.lo, {w}, f{slot});")
        elif op in ("mul", "mul:hi", "imul:lo", "imul:hi"):
            body.append(f"V {r} = mulv({('mul', 'mul:hi', 'imul:lo', 'imul:hi').index(op)}, {A}.lo, {C}.lo, {w});")
        elif op in ("div", "div:hi", "idiv", "idiv:hi"):
            body.append(f"V {r}; if (!divv({('div', 'div:hi', 'idiv', 'idiv:hi').index(op)}, {A}, {B}, {C}, {w}, {r})) return false;")
        elif op == "bt":
            flags.add(slot)
            word = f"{A}.lo" if x < 64 else f"{A}.hi"
            bit = x % 64
            update = {1: f"{r}.{'lo' if x < 64 else 'hi'} |= 1ull << {bit};", 0: f"{r}.{'lo' if x < 64 else 'hi'} &= ~(1ull << {bit});", 2: ""}[y]
            body.append(f"Fl f{slot} = NOF; f{slot}.cf = (u8)(({word} >> {bit}) & 1); V {r} = {A}; {update}")
        else:
            raise ValueError(op)
        return step

    def phi(arms, lines):
        """The one value among several that belongs to the path still alive."""
        slot = phis[0]
        phis[0] += 1
        arms = sorted(arms, key=lambda arm: arm[0] & -arm[0])
        pick = f"v{arms[-1][1]}"
        flag = fl(arms[-1][1])
        for m, s in reversed(arms[:-1]):
            pick = f"({test(m)}) ? v{s} : {pick}"
            flag = f"({test(m)}) ? {fl(s)} : {flag}"
        lines.append(f"V v{slot} = {pick};")
        if any(s in flags for _, s in arms):
            flags.add(slot)
            lines.append(f"Fl f{slot} = {flag};")
        return slot

    def one(e, lines):
        n = list(e.node)
        for at, arms in e.arms.items():
            if at == 7:
                n[7] = arms[0][1] if len(arms) == 1 else arms
            else:
                n[at] = arms[0][1] if len(arms) == 1 else phi(arms, lines)
        if OPS[n[0]] == "guard" and e.true and e.false:
            narrow = " ".join(f"alive{w} &= t ? ~0x{word(e.false, w):x}ull : ~0x{word(e.true, w):x}ull;" for w in words)
            lines.append(f"V v{e.slot} = mk(0); {{ bool bad = false; bool t = cond({n[5]}, {fl(n[2])}, bad); if (bad) return false; {narrow} }} if (!({test(e.mask)})) return false;")
        else:
            if OPS[n[0]] == "guard":
                n[6] = 1 if e.true else 0
            line(n, e.slot, lines)

    labels = [0]
    text = []
    at = 0
    gone = "if (!(" + " | ".join(f"alive{w}" for w in words) + ")) return false;"
    while at < len(entries):
        mask = entries[at].mask
        end = at
        while end < len(entries) and entries[end].mask == mask:
            end += 1
        if mask == everyone:
            for e in entries[at:end]:
                one(e, text)
        else:
            lines = []
            for e in entries[at:end]:
                inner = []
                one(e, inner)
                for k in range(e.node[8] if OPS[e.node[0]] == "fn" else 1):
                    s = e.slot + k
                    text.append(f"V v{s};")
                    inner = [item.replace(f"V v{s} =", f"v{s} =").replace(f"V v{s};", "") for item in inner]
                    if s in flags:
                        text.append(f"Fl f{s} = NOF;")
                        inner = [item.replace(f"Fl f{s} =", f"f{s} =").replace(f"Fl f{s};", "") for item in inner]
                lines += inner
            labels[0] += 1
            label = f"skip{labels[0]}"
            text.append(f"if ({test(mask)}) {{")
            text += [item.replace("return false;", f"{{ {clear(mask)} goto {label}; }}") for item in lines]
            text += ["}", f"{label}: ;", gone]
        at = end
    for k, arms in enumerate(roots):
        slot = arms[0][1] if len(arms) == 1 else phi(arms, text)
        text.append(f"out[{k}] = v{slot};")
    return code + body + text + ["return true;", "}"]


def main():
    source, target = Path(sys.argv[1]), Path(sys.argv[2])
    target.mkdir(parents=True, exist_ok=True)
    regions, domain, base, known, tables, mains = parse(source)
    tables, mains = unite(tables, mains)
    protos, functions = [], []
    global SLIM
    words, debug = slim_words(tables, mains, len(base), source)
    SLIM = {w: i for i, w in enumerate(words)}
    plain = bool(os.environ.get("NIOH3_PLAIN"))
    needs = {} if plain else demands(tables, mains)
    sure = None if plain or debug else known
    groups, calls = families(tables, needs)
    tops, _ = families([mains], {(0, k): v for (t, k), v in needs.items() if t == -1})
    for f, group in enumerate(groups):
        functions.append("\n".join(emit(f"sub{f}", group, protos, sure)))
    for f, group in enumerate(tops):
        functions.append("\n".join(emit(f"main{f}", group, protos, sure)))
    head = []
    for t, parts in enumerate(calls):
        tries = " ".join(f"if (sub{f}(c, a, o, {', '.join(f'0x{word(m, w):x}ull' for w in range((groups[f][5].bit_length() + 63) // 64))})) return true;" for f, m in parts)
        head.append(f"__noinline__ __device__ bool table{t}(const Ctx& c, const V* a, V* o) {{ {tries} return false; }}")
    proto_tables = [f"__noinline__ __device__ bool table{t}(const Ctx& c, const V* a, V* o);" for t in range(len(tables))]
    run = "__device__ bool run_main(const Ctx& c, u32& bits) { V o[2]; V none[1] = {{0, 0}}; " + " ".join(f"if (main{k}(c, none, o)) {{ bits = (u32)o[0].lo; return true; }}" for k in range(len(tops))) + " return false; }"
    spans = f"#define SLIM_WORDS {len(words)}@#define TOUCH {int(debug)}@#define PROFILE 0@#define PROF_N 1@static const u16 SLIM_LIST[] = {{{', '.join(map(str, words))}}};@".replace("@", NL_C) + "static const Span DOM_SPANS[] = {" + ", ".join(f"{{0x{s:x}ull, 0x{e:x}ull, {o}ull}}" for s, e, o in domain) + "};\n"
    spans += "static const Span REGIONS[] = {" + ", ".join(f"{{0x{s:x}ull, {n}ull, {o}ull}}" for s, n, o in sorted(regions)) + "};\n"
    inc = Path(__file__).with_name("path_kernel.inc").read_text(encoding="ascii")
    text = "\n".join([PRELUDE, spans] + proto_tables + protos + functions + head + [run, inc])
    (target / "path_kernel.cu").write_text(text, encoding="ascii", newline="\n")
    (target / "dom_base.bin").write_bytes(base)
    (target / "dom_known.bin").write_bytes(known)
    for name in ("arena.bin", "mapped.bin", "tables.bin", "frame.bin", "characters.bin", "pool.bin", "gpu_cases.txt", "gpu_setup.txt", "hits.txt"):
        shutil.copyfile(source / name, target / name)
    print("generated", len(functions), "functions", len(text) // 1024, "KB domain", len(base), "slim words", len(words), "instrumented" if debug else "")


main()
