"""Generate a CUDA kernel from an exported path bundle: each program becomes straight C, a failed guard returns false and the next path runs.

Not linked into any product. The kernel must match the bundle's vectors bit for bit before its speed means anything. argv: export_dir out_dir
"""
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
INLINE = "__noinline__" if os.environ.get("NIOH3_PLAIN") else "__forceinline__"


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


def emit(name, program, out, need=None, known=None):
    consts, root, written, nodes = program
    need = need or {}
    body = []
    head = f"{INLINE if name.startswith('sub') else '__noinline__'} __device__ bool {name}(const Ctx& c, const V* args, V* out)"
    out.append(head + ";")
    code = [head + " {"]
    if written:
        tests = " || ".join(f"(a < 0x{e:x}ull && 0x{s:x}ull < e)" for s, e in written)
        code.insert(0, f"HD bool {name}_w(u64 a, u64 e) {{ return {tests}; }}")
    for i, v in enumerate(consts):
        body.append(f"const V v{i} = {lit(v)};")
    slot = len(consts)
    flags = set()

    def fl(k):
        return f"f{k}" if k in flags else "NOF"

    for n in nodes:
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
            body.append(f"V a{slot}[{max(1, len(extra))}] = {{{items}}}; V o{slot}[{y}]; if (!table{x}(c, a{slot}, o{slot})) return false;")
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
                slot += step
                continue
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
        slot += step
    for k, s in enumerate(root):
        body.append(f"out[{k}] = v{s};")
    return code + body + ["return true;", "}"]


def main():
    source, target = Path(sys.argv[1]), Path(sys.argv[2])
    target.mkdir(parents=True, exist_ok=True)
    regions, domain, base, known, tables, mains = parse(source)
    protos, functions = [], []
    global SLIM
    words, debug = slim_words(tables, mains, len(base), source)
    SLIM = {w: i for i, w in enumerate(words)}
    plain = bool(os.environ.get("NIOH3_PLAIN"))
    needs = {} if plain else demands(tables, mains)
    sure = None if plain or debug else known
    for t, programs in enumerate(tables):
        for k, q in enumerate(programs):
            functions.append("\n".join(emit(f"sub{t}_{k}", q, protos, needs.get((t, k)), sure)))
    for k, q in enumerate(mains):
        functions.append("\n".join(emit(f"main{k}", q, protos, needs.get((-1, k)), sure)))
    profile = bool(os.environ.get("NIOH3_PROFILE"))
    starts, at = [], 0
    for programs in tables:
        starts.append(at)
        at += len(programs)
    main_at = at

    def index(t, k):
        return starts[t] + k

    def attempt(call, i, then):
        if not profile:
            return f"if ({call}) {{ {then} }}"
        return f"atomicAdd(c.prof + {2 * i}, 1u); if ({call}) {{ atomicAdd(c.prof + {2 * i + 1}, 1u); {then} }}"
    head = []
    for t, programs in enumerate(tables):
        head.append(f"__noinline__ __device__ bool table{t}(const Ctx& c, const V* a, V* o) {{ " + " ".join(attempt(f"sub{t}_{k}(c, a, o)", index(t, k), "return true;") for k in range(len(programs))) + " return false; }")
    proto_tables = [f"__noinline__ __device__ bool table{t}(const Ctx& c, const V* a, V* o);" for t in range(len(tables))]
    run = "__device__ bool run_main(const Ctx& c, u32& bits) { V o[2]; V none[1] = {{0, 0}}; " + " ".join(attempt(f"main{k}(c, none, o)", main_at + k, "bits = (u32)o[0].lo; return true;") for k in range(len(mains))) + " return false; }"
    spans = f"#define SLIM_WORDS {len(words)}@#define TOUCH {int(debug)}@#define PROFILE {int(profile)}@#define PROF_N {main_at + len(mains)}@static const u16 SLIM_LIST[] = {{{', '.join(map(str, words))}}};@".replace("@", NL_C) + "static const Span DOM_SPANS[] = {" + ", ".join(f"{{0x{s:x}ull, 0x{e:x}ull, {o}ull}}" for s, e, o in domain) + "};\n"
    spans += "static const Span REGIONS[] = {" + ", ".join(f"{{0x{s:x}ull, {n}ull, {o}ull}}" for s, n, o in sorted(regions)) + "};\n"
    inc = Path(__file__).with_name("path_kernel.inc").read_text(encoding="ascii")
    text = "\n".join([PRELUDE, spans] + proto_tables + protos + functions + head + [run, inc])
    (target / "path_kernel.cu").write_text(text, encoding="ascii", newline="\n")
    (target / "dom_base.bin").write_bytes(base)
    (target / "dom_known.bin").write_bytes(known)
    for name in ("arena.bin", "mapped.bin", "tables.bin", "frame.bin", "characters.bin", "pool.bin", "gpu_cases.txt", "gpu_setup.txt"):
        shutil.copyfile(source / name, target / name)
    print("generated", len(functions), "functions", len(text) // 1024, "KB domain", len(base), "slim words", len(words), "instrumented" if debug else "")


main()
