// The equipment pipeline on the GPU: pipeline.rs line for line, so a difference is always a bug here, then the generated formula paths.
// path_cuda.py pastes this after the generated functions. Compile with --fmad=false: a fused multiply-add is not what the game's SSE does.

#include <map>
#include <algorithm>
#include <cstring>
typedef int32_t i32;
#define ITEM 0xE8u
#define BANK 0xF70u
#define ABSENT 0xffffffffu
#define PLUS8_WEAPON 35u
#define BASE_DEFENSE_SOURCES 378ll
#define TRY(x) do { if (!(x)) return false; } while (0)

struct Keyed { const u8* rows; u32 size; u32 stride; const u32* direct; u32 direct_n; const u32* sparse; u32 sparse_n; };
struct Klass { u32 kind, start, n; };
struct Recipe { u32 kind, offset, width, has_scale; float scale; u32 pad; u64 literal; };
struct Entry { u16 key, dup; u32 base; };
struct Character { const u8* inv; const Entry* core[2]; u32 core_n[2]; i32 mode; u32 active, cap, plus_cap; i32 style; u32 unarmed, level; i32 core_mode; };
#define CAND_SLOTS 10
struct Limits { u32 on; float max_load; u32 min_defense, min_health; };
struct Cand { i32 attrs[7]; i32 slot[CAND_SLOTS]; };
struct S;
struct G {
    Keyed items, effects, groups, rarity, sides, special, core300, core340;
    const u8* level_rows; const u8* base70; const u8* power90; const u8* base90; const u8* grade_rows;
    u32 base70_rows, fam_steps, fam_bonus_steps, alternate_n, alternate_default;
    const u32* fam; const u32* alternate; const u32* grade_index; const Klass* klass; const Recipe* recipes;
    i32 families[2], slots5[5], slots8[8];
    float c_percent, c_4688, c_one, c_half, c_tenth, c_atten, c_4894, c_4080, opt_recovery, opt_e40f, opt_22af, opt_1f32, opt_d1f, opt_dc22, opt_d84f;
    i32 aeac_int;
    const u8* stat; const u8* d0; const u8* s_base; const u8* d_base;
    u64 stat_addr, d_addr, main_addr, ext_addr, owner, attr, dest;
    u32 main8, hp_base, gain, defense, op_n;
    const Entry* ops_unused; const u16* op_group; const i64* op_value;
    Character c;
    const u8* pool; const u8* dom_base; const u8* dom_known; const u32* word_mask; const u32* stat_live; const u32* d_live; const u32* pred_live; const u32* stat_out; const u32* d_out; u32 stat_live_n, d_live_n, pred_live_n, stat_out_n, d_out_n; u32 body_live[12];
    Ctx ctx; u32 dom_size;
    const S* seed;
    Limits limits;
};
struct Hit { u32 bits; u32 space; u64 index; };
struct Space { int pipeline_only; int id; int n; int slot[CAND_SLOTS]; u32 start[CAND_SLOTS]; u32 radix[CAND_SLOTS]; i32 attrs[7]; float keep; };
struct __align__(8) S {
    u8 stat[0x900], d[0xB00], pred[0x900];
    u32 dom[SLIM_WORDS], known[SLIM_WORDS];
    Entry maps[2][300];
    u8 sides[2][20][68];
    u32 map_n[2], side_n[2], thresholds[7];
    i32 transformed[7], mode;
};

// Candidate index of an equipment slot: 4..12 are armor and accessories, the held melee weapon is last. -1 for slots a candidate cannot change.
__device__ __host__ inline int cand_index(u32 slot) { return slot == 0 ? 9 : slot >= 4 && slot < 13 ? (int)slot - 4 : -1; }
HD u32 u16_at(const u8* b, u32 o) { const u8* p = b + o; if (((uintptr_t)p & 1) == 0) return *(const u16*)p; return (u32)p[0] | (u32)p[1] << 8; }
HD u32 u32_at(const u8* b, u32 o) { const u8* p = b + o; if (((uintptr_t)p & 3) == 0) return *(const u32*)p; return (u32)p[0] | (u32)p[1] << 8 | (u32)p[2] << 16 | (u32)p[3] << 24; }
HD i32 i32_at(const u8* b, u32 o) { return (i32)u32_at(b, o); }
__device__ __host__ inline float bits_f(u32 v) { union { u32 u; float x; } c; c.u = v; return c.x; }
HD u32 f_bits(float x) { union { u32 u; float x; } c; c.x = x; return c.u; }
HD float f32_at(const u8* b, u32 o) { return bits_f(u32_at(b, o)); }
HD void put_u16(u8* b, u32 o, u32 v) { u8* p = b + o; if (((uintptr_t)p & 1) == 0) { *(u16*)p = (u16)v; return; } p[0] = (u8)v; p[1] = (u8)(v >> 8); }
HD void put_u32(u8* b, u32 o, u32 v) { u8* p = b + o; if (((uintptr_t)p & 3) == 0) { *(u32*)p = v; return; } p[0] = (u8)v; p[1] = (u8)(v >> 8); p[2] = (u8)(v >> 16); p[3] = (u8)(v >> 24); }
HD void put_u64(u8* b, u32 o, u64 v) { put_u32(b, o, (u32)v); put_u32(b, o + 4, (u32)(v >> 32)); }
HD void put_f32(u8* b, u32 o, float v) { put_u32(b, o, f_bits(v)); }
HD i32 cvtt32(float x) { return finite(x) && x >= -2147483648.0f && x < 2147483648.0f ? (i32)x : (i32)0x80000000; }
HD i64 cvtt64(float x) { return finite(x) && x >= -9223372036854775808.0f && x < 9223372036854775808.0f ? (i64)x : (i64)0x8000000000000000ull; }
HD int selected_map(i32 context, i32 mode) { return (context == -1 ? mode : context) == 1 ? 1 : 0; }
HD i32 imax(i32 a, i32 b) { return a > b ? a : b; }
HD i32 imin(i32 a, i32 b) { return a < b ? a : b; }
HD i32 clamp999(i32 v) { return v < 1 ? 1 : v > 999 ? 999 : v; }

HD u32 k_index(const Keyed& k, u32 key) {
    if (k.sparse_n == 0) return key < k.direct_n ? k.direct[key] : ABSENT;
    u32 lo = 0, hi = k.sparse_n;
    while (lo < hi) { u32 m = (lo + hi) / 2; u32 x = k.sparse[2 * m]; if (x == key) return k.sparse[2 * m + 1]; if (x < key) lo = m + 1; else hi = m; }
    return ABSENT;
}
HD const u8* k_row(const Keyed& k, u32 key) { u32 i = k_index(k, key); return i == ABSENT ? nullptr : k.rows + 8 + (u64)i * k.stride; }
HD const u8* item_ptr(const G& g, const Cand& q, u32 bank, u32 slot) {
    if (bank == g.c.active) { int a = cand_index(slot); if (a >= 0 && q.slot[a] >= 0) return g.pool + (u64)q.slot[a] * ITEM; }
    return g.c.inv + bank * BANK + slot * ITEM;
}
HD u32 field70(const G& g, u32 index, u32 off) { u32 i = umin(index, g.base70_rows - 1); return u16_at(g.base70, i * 0x94 + off); }
HD u32 field90(const G& g, i32 index, u32 off) { return u16_at(g.power90, (u32)clamp999(index) * 32 + off); }

HD bool familiarity(const G& g, u32 raw, u8 rarity, u32& out) {
    out = 0;
    if (rarity > 5) return true;
    u32 r = rarity, steps = 0;
    for (u32 i = 0; i < g.fam_steps; i++) { if (raw >= g.fam[r * g.fam_steps + i]) steps++; else break; }
    if (steps == 0) return true;
    if (steps > g.fam_bonus_steps) return false;
    out = g.fam[6 * g.fam_steps + r * g.fam_bonus_steps + steps - 1];
    return true;
}
struct Effective { u32 level, plus; };
HD bool effective(const G& g, const u8* item, bool use_familiarity, Effective& e) {
    u8 rarity = item[0x30];
    u32 flags = u32_at(item, 0x18);
    u32 base = flags & 0x200000 ? 0 : umin(u16_at(item, 6), 180);
    u32 fam = 0;
    if (use_familiarity) TRY(familiarity(g, u32_at(item, 0x14), rarity, fam));
    u32 plus = umin(u16_at(item, 0xA), g.c.cap), add = 0;
    if (rarity <= 5) {
        const u8* row = k_row(g.rarity, rarity);
        if (row) add = u32_at(row, 0x14);
        if ((flags & 0x40000) == 0 && rarity == 5) plus += (u32)g.aeac_int;
    }
    e.level = base + fam + add;
    e.plus = plus;
    return true;
}
HD bool selection(const G& g, const u8* item, const u8*& original, const u8*& chosen) {
    u32 key = u16_at(item, 0);
    if (key == 0) return false;
    original = k_row(g.items, key);
    if (!original) return false;
    u32 selected = key;
    if ((signed char)item[0x30] >= 4) { u32 r = u16_at(original, 0x160); if (r != 0) selected = r; }
    chosen = k_row(g.items, selected);
    return chosen != nullptr;
}
HD u32 flat_group(const G& g, const u8* item, u32 group) {
    u32 total = 0;
    for (u32 i = 0; i < 7; i++) {
        u32 effect = u32_at(item, 0x38 + 24 * i);
        if (effect == ABSENT) continue;
        const u8* row = k_row(g.effects, effect);
        if (row && u16_at(row, 2) == group) total += u32_at(item, 0x3C + 24 * i);
    }
    return total;
}
HD u32 requirement_delta(const G& g, const u8* item) {
    if ((signed char)item[0x30] < 3) return 0;
    int count = (item[0x31] == 1) + (item[0x32] == 1);
    float coefficient = (float)count * g.c_half;
    u32 plus = umin(u16_at(item, 0xA), g.c.cap);
    return (u32)cvtt64((float)field70(g, plus, 0x62) * coefficient);
}
HD bool requirements(const G& g, const i32* raw, const u8* original, const u8* item) {
    u32 kind = u32_at(original, 0x60);
    u32 column = (signed char)item[0x30] >= 5 && kind != 0x2D32 ? 1 : 0;
    u32 delta = requirement_delta(g, item);
    for (u32 i = 0; i < 7; i++) {
        u32 b = original[0x18C + i * 2 + column];
        i32 needed = b != 0 ? (i32)(b + delta) : 0;
        if (needed > raw[i]) return false;
    }
    return true;
}
HD bool weapon_fields(const G& g, const u8* item, i32 style, bool secondary, i32& out_base, i32& out_coefficient, u32& level_field) {
    const u8* original; const u8* p;
    TRY(selection(g, item, original, p));
    bool alt = secondary && u32_at(original, 0x58) != 0;
    u32 base = u32_at(p, alt ? 0x14 : 0x10), coefficient = u32_at(p, alt ? 0xD8 : 0xD4);
    int off = style == 0 ? (alt ? 0x20 : 0x18) : style == 2 ? (alt ? 0x38 : 0x30) : -1;
    if (off >= 0) { base += u32_at(p, off); coefficient += u32_at(p, off + 4); }
    level_field = 0x1E;
    if (alt) {
        u32 selector = u32_at(original, 0x58);
        level_field = g.alternate_default;
        for (u32 i = 0; i < g.alternate_n; i++) if (g.alternate[2 * i] == selector) { level_field = g.alternate[2 * i + 1]; break; }
    }
    out_base = imax((i32)base, 0);
    out_coefficient = imax((i32)coefficient, 0);
    return true;
}
HD bool special_bound(const G& g, const u8* item, i32 base, i32 reference, u32 category, u32 slope_at, u32 intercept_at, i32& out) {
    if (g.special.size == 0) return false;
    const u8* row = k_row(g.special, category);
    if (!row) { out = base; return true; }
    float slope = f32_at(row, slope_at), intercept = f32_at(row, intercept_at);
    if (!finite(slope) || !finite(intercept)) return false;
    u32 flags = u32_at(item, 0x18);
    u32 level = flags & 0x200000 ? 0 : umin(u16_at(item, 6), 180);
    float term = (float)level * (slope * g.c_percent);
    float product = (term + intercept) * (float)reference;
    if (!finite(product)) return false;
    out = imin(base, cvtt32(product));
    return true;
}
HD bool weapon_scalar(const G& g, const u8* item, bool admitted, bool secondary, u32& out) {
    const u8* original; const u8* chosen;
    TRY(selection(g, item, original, chosen));
    i32 base, coefficient; u32 level_field;
    TRY(weapon_fields(g, item, g.c.style, secondary, base, coefficient, level_field));
    i32 bounded = base;
    if (u32_at(item, 0x18) & 0x40000) {
        u32 linked_key = u16_at(chosen, 0x160);
        i32 reference = base; u32 category = u16_at(chosen, 0x152);
        const u8* linked = k_row(g.items, linked_key);
        if (linked) { reference = i32_at(linked, 0x10); category = u16_at(linked, 0x152); }
        TRY(special_bound(g, item, base, reference, category, 4, 0x10, bounded));
    }
    Effective e;
    TRY(effective(g, item, true, e));
    u32 level = umin(e.level, 190), plus = umin(e.plus, g.c.plus_cap);
    u32 product = (field70(g, level, level_field) + field70(g, plus, 0x20)) * (u32)coefficient;
    u32 flat = admitted ? flat_group(g, item, 0x8459) : 0;
    out = product / 1000 + (u32)bounded + flat;
    return true;
}
HD bool auxiliary(const G& g, const u8* item, i32 style, bool secondary, u32& out) {
    const u8* original; const u8* chosen;
    TRY(selection(g, item, original, chosen));
    u32 parameter = secondary ? 0xE0 : 0xDC, style0 = secondary ? 0x2C : 0x28, style2 = secondary ? 0x44 : 0x40, slope = secondary ? 0xC : 8, intercept = secondary ? 0x18 : 0x14;
    u32 value = u32_at(chosen, parameter);
    if (style == 0) value += u32_at(chosen, style0); else if (style == 2) value += u32_at(chosen, style2);
    i32 base = (i32)value, bounded = base;
    if (u32_at(item, 0x18) & 0x40000) {
        u32 linked_key = u16_at(original, 0x160);
        i32 reference = base; u32 category = u16_at(original, 0x152);
        const u8* linked = k_row(g.items, linked_key);
        if (linked) { reference = i32_at(linked, parameter); category = u16_at(linked, 0x152); }
        TRY(special_bound(g, item, base, reference, category, slope, intercept, bounded));
    }
    out = (u32)imax(bounded, 0);
    return true;
}
HD void default_core(const G& g, const u8* original, u32* coefficients, u32& mask) {
    const u8* row = k_row(g.core300, u32_at(original, 0x58));
    if (row) { for (u32 i = 0; i < 3; i++) coefficients[i] = u32_at(row, 0x28 + 4 * i); mask = u32_at(row, 0x34); return; }
    row = k_row(g.core340, u32_at(original, 0x5C));
    if (row) { for (u32 i = 0; i < 3; i++) coefficients[i] = u32_at(row, 0x1C + 4 * i); mask = u32_at(row, 0x28); return; }
    coefficients[0] = coefficients[1] = coefficients[2] = 0; mask = 0;
}
HD bool local_mask(const G& g, const u8* item, u32& out) {
    const u8* original = k_row(g.items, u16_at(item, 0));
    if (!original) return false;
    u32 c[3], native;
    default_core(g, original, c, native);
    out = item[0x33] != 0 ? item[0x33] : native;
    return true;
}
HD bool core_mask(const G& g, const Cand& q, const u8* item, const u8* original, u32 slot, u32& mask) {
    TRY(local_mask(g, item, mask));
    u32 flags = u32_at(original, 0xB0);
    for (u32 family = 0; family < 2; family++) {
        if ((flags & (1u << (11 + family))) == 0) continue;
        u32 key = family == 0 ? 0xD7BF : 0xCB4B;
        int which = g.c.core_mode == 1 ? 1 : 0;
        i32 value = -1;
        for (u32 i = 0; i < g.c.core_n[which]; i++) { Entry e = g.c.core[which][i]; if (e.key == key) { value = (i32)(e.base + e.dup); break; } }
        if (value >= 0) {
            const u8* other = item_ptr(g, q, family ^ 1, slot);
            if (k_index(g.items, u16_at(other, 0)) != ABSENT) { TRY(local_mask(g, other, mask)); break; }
        }
    }
    return true;
}
HD bool core_bonus(const G& g, const Cand& q, const i32* transformed, const u8* item, const u8* original, u32 slot, u32& out) {
    u32 coefficients[3], native, mask;
    default_core(g, original, coefficients, native);
    TRY(core_mask(g, q, item, original, slot, mask));
    i32 values[7];
    for (u32 j = 0; j < 7; j++) values[j] = mask & (1u << j) ? transformed[j] : 0;
    for (int a = 1; a < 7; a++) { i32 v = values[a]; int b = a - 1; while (b >= 0 && values[b] < v) { values[b + 1] = values[b]; b--; } values[b + 1] = v; }
    u32 total = 0;
    for (u32 k = 0; k < 3; k++) total += coefficients[k] * (u32)values[k];
    out = total / 100;
    return true;
}
HD bool diminished(const G& g, u32 base_u32, u32 bonus, u32& out) {
    float base = (float)base_u32, remaining = (float)bonus;
    float tranche = base * g.c_tenth, half = base * g.c_half;
    if (remaining <= 0.0f) { out = 0; return true; }
    if (remaining <= half) { out = (u32)cvtt64(remaining + 0.0f); return true; }
    remaining -= half;
    float acc = half + 0.0f;
    for (int i = 0; i < 1000; i++) {
        remaining *= g.c_atten;
        if (remaining < 1.0f) { out = (u32)cvtt64(acc); return true; }
        if (remaining <= tranche) { acc += remaining; out = (u32)cvtt64(acc); return true; }
        acc += tranche;
        remaining -= tranche;
    }
    return false;
}
HD bool pair_(const G& g, u32 base, u32 bonus, u32& out) {
    u32 d;
    TRY(diminished(g, base, bonus, d));
    float summed = (float)(base + d);
    if (!finite(summed)) return false;
    out = (u32)cvtt64(summed);
    return true;
}
HD bool side_entry(const G& g, u8* entry, u32 item_key, i32 level, u8 special, i32 context) {
    const u8* parameter = k_row(g.sides, u32_at(entry, 0));
    if (!parameter) return false;
    u32 count = u16_at(entry, 4);
    bool fixed = u32_at(parameter, 0x10) != 0;
    if (!fixed && count > 7) return false;
    u32 levels[7], items[7];
    for (u32 i = 0; i < 7; i++) { levels[i] = u16_at(entry, 0x10 + 2 * i); items[i] = u16_at(entry, 0x2C + 2 * i); }
    if (fixed) {
        int slot = -1;
        for (int i = 0; i < 7; i++) if (u16_at(parameter, 0x14 + 4 * i) == item_key) { slot = i; break; }
        if (slot < 0) return true;
        u32 old = levels[slot];
        if (old == 0 || (i32)old < level) {
            levels[slot] = (u16)level;
            items[slot] = item_key;
            if (old == 0) { count = (count + 1) & 65535; put_u16(entry, 4, count); }
            if (special != 0 && (old == 0 || entry[0x3A + slot] == 0)) { put_u16(entry, 6, u16_at(entry, 6) + 1); entry[0x3A + slot] = 1; }
        }
    } else {
        int free_slot = -1, last = -1;
        for (int i = 0; i < 7; i++) if (levels[i] == 0) { free_slot = i; break; }
        for (int i = 6; i >= 0; i--) if (levels[i] > 0) { last = i; break; }
        int s = free_slot >= 0 ? free_slot : last;
        if (s >= 0) {
            levels[s] = (u16)level;
            items[s] = item_key;
            if (free_slot >= 0) { count = (count + 1) & 65535; put_u16(entry, 4, count); }
        }
        if (count > 7) return false;
        int n = (int)count;
        u32 ranks[7] = {0, 0, 0, 0, 0, 0, 0};
        for (int i = 0; i < n; i++) { u32 r = k_index(g.items, items[i]); ranks[i] = r == ABSENT ? 0 : r; }
        bool settled = false;
        for (int pass = 0; pass < 1000; pass++) {
            bool changed = false;
            for (int left = 0; left < n; left++) {
                for (int right = left + 1; right < n; right++) {
                    bool better = levels[right] > levels[left] || (levels[right] == levels[left] && ranks[right] < ranks[left]);
                    if (better) changed = true;
                    if (changed) {
                        u32 t = levels[left]; levels[left] = levels[right]; levels[right] = t;
                        t = items[left]; items[left] = items[right]; items[right] = t;
                        t = ranks[left]; ranks[left] = ranks[right]; ranks[right] = t;
                        break;
                    }
                }
            }
            if (!changed) { settled = true; break; }
        }
        if (!settled) return false;
    }
    for (u32 i = 0; i < 7; i++) { put_u16(entry, 0x10 + 2 * i, levels[i]); put_u16(entry, 0x2C + 2 * i, items[i]); }
    put_u32(entry, 0xC, (u32)context);
    u32 ordered[7] = {0, 0, 0, 0, 0, 0, 0};
    int k = 0;
    for (int i = 0; i < 7; i++) if (levels[i] > 0) ordered[k++] = levels[i];
    for (int a = 1; a < k; a++) { u32 v = ordered[a]; int b = a - 1; while (b >= 0 && ordered[b] < v) { ordered[b + 1] = ordered[b]; b--; } ordered[b + 1] = v; }
    u32 sum = 0;
    for (u32 index = 0; index < 7; index++) {
        sum += ordered[index];
        u32 value = index < count ? sum / (index + 1) : u16_at(entry, 0x1C + 2 * index);
        put_u16(entry, 0x1E + 2 * index, value);
    }
    return true;
}
HD void aggregate(const G& g, S& s, i32 context, u32 key, u32 value) {
    if (key == 0) return;
    const u8* row = k_row(g.groups, key);
    if (!row) return;
    int t = -1;
    switch (u16_at(row, 0xC)) {
    case 0xF8B5: t = 0; break; case 0xECC5: t = 1; break; case 0x4329: t = 2; break; case 0x3711: t = 3; break;
    case 0x89DE: t = 4; break; case 0x35BE: t = 5; break; case 0xB51B: t = 6; break;
    }
    if (t >= 0 && (i32)value > (i32)s.thresholds[t]) return;
    int bank = selected_map(context, s.mode);
    Entry* entries = s.maps[bank];
    u32 n = s.map_n[bank], i = 0;
    while (i < n && entries[i].key != key) i++;
    if (i == n) {
        if (n < 256) { entries[n].key = (u16)key; entries[n].base = value; entries[n].dup = 0; s.map_n[bank] = n + 1; }
        return;
    }
    Entry& e = entries[i];
    if (key == 0x38BA) {
        e.base = umax(e.base + e.dup, value);
    } else if (row[0x50] & 4) {
        e.base = umax(e.base, value);
        e.dup = (u16)(e.dup + 1);
    } else {
        u32 d = e.dup;
        e.base = e.base + d + value;
        i32 cap = i32_at(row, 4);
        if (cap >= 0 && (i64)(u32)(e.base + d) > (i64)cap) e.base = (u32)cap;
    }
}
HD bool collect(const G& g, S& s, const u8* item, bool reserve, i32 context, u32 mask) {
    u32 key = u16_at(item, 0);
    const u8* parameter = k_row(g.items, key);
    if (!parameter) return true;
    u8 item_type = parameter[0x182];
    for (u32 slot = 0; slot < 7; slot++) {
        u32 effect = u32_at(item, 0x38 + slot * 24), value = u32_at(item, 0x3C + slot * 24);
        if (effect == ABSENT) continue;
        const u8* effect_row = k_row(g.effects, effect);
        if (!effect_row) continue;
        u32 group = u16_at(effect_row, 2);
        if (group == 0) continue;
        const u8* row = k_row(g.groups, group);
        if (!row) continue;
        if (!reserve && (((item_type == 1 || item_type == 2) && group == 0x8459) || (item_type == 3 && group == 0xDA05))) continue;
        u32 side_key = u32_at(row, 8);
        if (side_key != 0) {
            u32 flags = u32_at(item, 0x18);
            i32 level = flags & 0x200000 ? 0 : imin((i32)u16_at(item, 6), 180);
            int bank = selected_map(context, s.mode);
            u32 n = s.side_n[bank], found = n;
            for (u32 i = 0; i < n; i++) if (u32_at(s.sides[bank][i], 0) == side_key) { found = i; break; }
            if (found == n) {
                if (n == 20) continue;
                for (u32 i = 0; i < 68; i++) s.sides[bank][n][i] = 0;
                put_u32(s.sides[bank][n], 0, side_key);
                s.side_n[bank] = n + 1;
            }
            TRY(side_entry(g, s.sides[bank][found], key, level, (u8)((flags >> 19) & 1), context));
            continue;
        }
        if (reserve) { if (u16_at(row, 0xC) != 0x38BA) continue; }
        else if ((mask & (1u << slot)) == 0) continue;
        aggregate(g, s, context, group, value);
    }
    return true;
}
HD float fraction(const G& g, const u8* original, u32 off) { u32 v = u32_at(original, off); return v != 0 ? (float)v * g.c_percent : g.c_one; }
HD bool prepared(const G& g, const Cand& q, S& s) {
    u32 bank = g.c.active;
    const u32 slots[5] = {0, 1, 2, 3, 13};
    for (u32 rec = 0; rec < 5; rec++) {
        u32 slot = slots[rec];
        const u8* item = item_ptr(g, q, bank, slot);
        u32 key = u16_at(item, 0);
        bool known = key != 0 && k_index(g.items, key) != ABSENT;
        u8* r = s.stat + 0x24 + 64 * rec;
        if (rec == 4 && !known) continue;
        if (!known) {
            put_u32(r, 0, g.c.unarmed); put_u32(r, 4, g.c.unarmed); put_u32(r, 0x18, ABSENT); put_u32(r, 8, 0); put_u32(r, 0x34, 0);
            put_u32(r, 0xC, 2); put_u32(r, 0x10, 20); put_u32(r, 0x14, 10); put_u32(r, 0x38, 13);
            put_u64(r, 0x1C, 0); put_u64(r, 0x24, 0); put_u64(r, 0x2C, 0);
            put_u16(r, 0x3C, 0x100);
            continue;
        }
        const u8* original = k_row(g.items, key);
        bool admitted = requirements(g, q.attrs, original, item);
        u32 primary, secondary, core, first, second, aux0, aux1;
        TRY(weapon_scalar(g, item, admitted, false, primary));
        TRY(weapon_scalar(g, item, admitted, true, secondary));
        TRY(core_bonus(g, q, s.transformed, item, original, slot, core));
        TRY(pair_(g, primary, core, first));
        TRY(pair_(g, secondary, core, second));
        TRY(auxiliary(g, item, g.c.style, false, aux0));
        TRY(auxiliary(g, item, g.c.style, true, aux1));
        put_u32(r, 0, first); put_u32(r, 8, 0); put_u32(r, 4, second); put_u32(r, 0xC, original[0x19B]); put_u32(r, 0x10, aux0);
        put_u32(r, 0x18, ABSENT); put_u32(r, 0x14, aux1); put_u32(r, 0x1C, 0);
        put_f32(r, 0x20, fraction(g, original, 0xE4));
        put_u32(r, 0x24, u32_at(original, 0xE8)); put_u32(r, 0x28, u32_at(original, 0xEC));
        put_f32(r, 0x2C, (float)u32_at(original, 0xF0) * g.c_percent);
        put_f32(r, 0x30, fraction(g, original, 0xF4));
        put_u32(r, 0x34, u32_at(original, 0x48)); put_u32(r, 0x38, original[0x19A]);
        r[0x3C] = (u8)((u32_at(original, 0xB0) >> 3) & 1);
        r[0x3D] = 0;
        u32 held = g.c.inv[bank * BANK + 0xF68] == 0 ? 1 : 0;
        u32 mask = slot == held || slot == 13 ? 255 : 0;
        if (admitted) TRY(collect(g, s, item, false, (i32)bank, mask));
    }
    for (u32 rec = 0; rec < 5; rec++) {
        const u8* item = item_ptr(g, q, bank ^ 1, slots[rec]);
        u32 key = u16_at(item, 0);
        if (key == 0) continue;
        const u8* p = k_row(g.items, key);
        if (!p) continue;
        if (requirements(g, q.attrs, p, item)) TRY(collect(g, s, item, true, (i32)(bank ^ 1), 255));
    }
    return true;
}
HD bool defense_of(const G& g, const u8* item, bool admitted, Effective e, u32& out) {
    const u8* o; const u8* p;
    TRY(selection(g, item, o, p));
    if (u32_at(item, 0x18) & 0x40000) return false;
    u32 level = umin(e.level, 190);
    u32 lc = field70(g, level, 0x26), pc = field70(g, umin(e.plus, g.c.plus_cap), 0x28);
    u32 raw_plus = umin(u16_at(item, 0xA), g.c.cap), delta1 = 0, delta2 = 0, kind = u32_at(p, 0x60);
    if ((signed char)item[0x30] >= 3) {
        float a = (float)((item[0x31] == 1) + (item[0x32] == 1)) * g.c_half, b = (float)((item[0x31] == 2) + (item[0x32] == 2)) * g.c_half;
        u32 oa = kind == 0x2B2F ? 0x68 : kind == 0x403B ? 0x66 : 0x64, ob = kind == 0x2B2F ? 0x70 : kind == 0x403B ? 0x6E : 0x6C;
        delta1 = (u32)cvtt64((float)field70(g, raw_plus, oa) * a);
        delta2 = (u32)cvtt64((float)field70(g, raw_plus, ob) * b);
    }
    u32 product = (lc + pc) * u32_at(p, 0xF8);
    u32 flat = admitted ? flat_group(g, item, 0xDA05) : 0;
    out = product / 1000 + u32_at(p, 0x50) + delta1 + delta2 + flat;
    return true;
}
HD u32 weight_of(const G& g, const u8* item, const u8* chosen) {
    int count = (signed char)item[0x30] >= 3 ? (item[0x31] == 2) + (item[0x32] == 2) : 0;
    float factor = (float)count * g.c_half;
    u32 plus = umin(u16_at(item, 0xA), g.c.cap);
    u32 delta = (u32)cvtt64((float)field70(g, plus, 0x6A) * factor);
    u32 total = u32_at(chosen, 0x98) + delta;
    return (u32)cvtt64((float)total);
}
HD bool body_field(const G& g, const u8* original, u32 q, Effective e, bool second, i32& out) {
    if (u32_at(original, 0x60) != 0x2D32) { out = (short)q; return true; }
    u32 level = umin(e.level, 190), plus = umin(e.plus, g.c.plus_cap);
    if (umax(level, plus) >= g.base70_rows) return false;
    u32 off = second ? 0x30 : 0x2C;
    float value = (float)(field70(g, level, off) + field70(g, plus, off + 2)) * g.c_4688 * (float)q;
    out = (short)(u16)cvtt32(value);
    return true;
}
__constant__ u32 BODY_FLOATS[12][3] = {{0x77c, 0x13a, 0}, {0x780, 0x13c, 0}, {0x784, 0x140, 0}, {0x788, 0x13e, 0}, {0x78c, 0x142, 0}, {0x790, 0x144, 0},
                                       {0x4dc, 0x146, 1}, {0x4fc, 0x148, 1}, {0x51c, 0x14c, 1}, {0x53c, 0x14a, 1}, {0x55c, 0x14e, 1}, {0x57c, 0x150, 1}};
HD bool body(const G& g, const Cand& q, S& s) {
    for (u32 i = 0; i < 12; i++) if (g.body_live[i] && !finite(f32_at(s.stat, BODY_FLOATS[i][0]))) return false;
    u8 refresh = s.stat[0x7D0];
    for (int pass = 0; pass < 2; pass++) {
        bool reserve = pass == 1;
        u32 bank = reserve ? g.c.active ^ 1 : g.c.active;
        for (u32 slot = 4; slot < 13; slot++) {
            const u8* item = item_ptr(g, q, bank, slot);
            u32 key = u16_at(item, 0);
            if (key == 0) continue;
            const u8* original = k_row(g.items, key);
            if (!original) continue;
            bool qualifies = requirements(g, q.attrs, original, item);
            if (!reserve) {
                const u8* o; const u8* chosen;
                TRY(selection(g, item, o, chosen));
                if (u32_at(item, 0x18) & 0x40000) return false;
                Effective e;
                TRY(effective(g, item, true, e));
                u32 defense;
                TRY(defense_of(g, item, qualifies, e, defense));
                put_u32(s.stat, 0x1F4 + 4 * (slot - 4), defense);
                u32 weight = weight_of(g, item, chosen);
                put_u32(s.stat, 0x1C8, u32_at(s.stat, 0x1C8) + defense);
                put_u32(s.stat, 0x218, u32_at(s.stat, 0x218) + u32_at(chosen, 0xFC));
                put_u32(s.stat, 0x7B8, u32_at(s.stat, 0x7B8) + weight);
                put_u16(s.stat, 0x21C, u16_at(s.stat, 0x21C) + u16_at(original, 0x138));
                for (u32 i = 0; i < 12; i++) {
                    if (!g.body_live[i]) continue;
                    i32 term;
                    TRY(body_field(g, original, u16_at(original, BODY_FLOATS[i][1]), e, BODY_FLOATS[i][2] != 0, term));
                    float v = (float)term + f32_at(s.stat, BODY_FLOATS[i][0]);
                    put_f32(s.stat, BODY_FLOATS[i][0], v);
                }
            }
            if (refresh == 0 && qualifies) TRY(collect(g, s, item, reserve, (i32)bank, reserve ? 0 : 255));
        }
    }
    return true;
}
HD bool slot15(const G& g, S& s) {
    const u8* item = g.c.inv + g.c.active * BANK + 15 * ITEM;
    u32 key = u16_at(item, 0);
    const u8* parameter = key != 0 ? k_row(g.items, key) : nullptr;
    if (parameter) {
        Effective e;
        TRY(effective(g, item, false, e));
        u32 level = umin(e.level, 190), plus = umin(e.plus, g.c.plus_cap);
        if (plus > 500 || umax(level, plus) >= g.base70_rows) return false;
        i32 product = (i32)((field70(g, level, 0x3E) + field70(g, plus, 0x40)) * (u32)imax(i32_at(parameter, 0xD4), 0));
        u32 attack = (u32)imax((i32)(u32_at(parameter, 0x10) + (u32)(product / 1000)), 1);
        u32 p2 = (field70(g, level, 0x42) + field70(g, plus, 0x44)) * u32_at(parameter, 0xF8);
        u32 defense = (u32)imax((i32)(u32_at(parameter, 0x50) + p2 / 1000), 1);
        for (u32 i = 0; i < 6; i++) for (u32 off = 0; off < 8; off += 4) { u32 at = 0x24 + 64 * i + off; put_u32(s.stat, at, u32_at(s.stat, at) + (attack & 65535)); }
        put_u32(s.stat, 0x1C8, u32_at(s.stat, 0x1C8) + defense);
        TRY(collect(g, s, item, false, g.c.mode, 255));
    }
    const u8* reserve = g.c.inv + (g.c.mode == 1 ? 0 : 1) * BANK + 15 * ITEM;
    key = u16_at(reserve, 0);
    if (key != 0 && k_index(g.items, key) != ABSENT) {
        for (u32 i = 0; i < 7; i++) {
            u32 effect = u32_at(reserve, 0x38 + 24 * i);
            if (effect == ABSENT) continue;
            const u8* row = k_row(g.effects, effect);
            if (!row) continue;
            u32 group = u16_at(row, 2);
            if ((group == 0xF79F || group == 0x62CB || group == 0x17B4) && k_index(g.groups, group) != ABSENT) aggregate(g, s, g.c.mode, group, u32_at(reserve, 0x3C + 24 * i));
        }
    }
    return true;
}
HD u32 curve(const G& g, const u8* reward, u32 coordinate) {
    u32 count = u32_at(g.level_rows, 4), index = umin(coordinate, 500);
    if (index >= count) return ABSENT;
    u32 mode = u32_at(reward, 12);
    i32 value = mode <= 4 ? (i32)u16_at(g.level_rows, 8 + index * 10 + 2 * mode) : 0;
    i32 product = (i32)((u32)value * (u32)i32_at(reward, 20));
    return u32_at(reward, 16) + (u32)(product / 1000);
}
HD bool set_rewards(const G& g, S& s) {
    i32 mode = g.c.mode;
    for (int f = 0; f < 2; f++) {
        int bank = selected_map(g.families[f], s.mode);
        u32 reduction = 0;
        for (u32 i = 0; i < s.map_n[bank]; i++) if (s.maps[bank][i].key == 0x38BA) { reduction = 1; break; }
        for (u32 index = 0; index < s.side_n[bank]; index++) {
            u8* raw = s.sides[bank][index];
            put_u32(raw, 8, reduction);
            if (u16_at(raw, 4) + reduction <= 1) continue;
            const u8* parameter = k_row(g.sides, u32_at(raw, 0));
            if (!parameter) continue;
            for (u32 reward = 0; reward < 7; reward++) {
                const u8* row = parameter + 0x30 + 24 * reward;
                u32 key = u16_at(row, 8), threshold = u32_at(row, 0);
                u8 flags = row[4];
                if (key == 0 || (i32_at(raw, 12) != mode && (flags & 2) == 0)) continue;
                u32 count = u16_at(raw, flags & 1 ? 6 : 4);
                if (count + reduction < threshold) continue;
                i32 coordinate = (i32)(threshold - reduction);
                i64 offset = coordinate > 0 ? 0x1C + 2 * (i64)coordinate : 0x1E;
                if (offset < 0 || offset + 2 > 68) return false;
                aggregate(g, s, mode, key, curve(g, row, u16_at(raw, (u32)offset)));
            }
        }
    }
    return true;
}
HD bool sample(const u8* row, u32 grade, float& out) {
    u32 count = u16_at(row, 0x52);
    u32 e = umin(grade >= count ? count - 1 : grade, 17);
    out = f32_at(row, 8 + 4 * e);
    return finite(out);
}
HD bool grade_of(const G& g, u32 key, i32 x, u32 grade, float& out) {
    u32 start = g.grade_index[2 * key], count = g.grade_index[2 * key + 1];
    if (count == 0) { out = 0.0f; return true; }
    const u8* rows = g.grade_rows + (u64)start * 84;
    u32 lo = 0, hi = count;
    while (lo < hi) { u32 mid = (lo + hi) / 2; if (i32_at(rows, mid * 84 + 4) < x) lo = mid + 1; else hi = mid; }
    if (lo == 0) return sample(rows, grade, out);
    if (lo == count) return sample(rows + (lo - 1) * 84, grade, out);
    float low, high;
    TRY(sample(rows + (lo - 1) * 84, grade, low));
    TRY(sample(rows + lo * 84, grade, high));
    i32 c0 = i32_at(rows, (lo - 1) * 84 + 4), c1 = i32_at(rows, lo * 84 + 4);
    i32 distance = (i32)((u32)x - (u32)c0), span = (i32)((u32)c1 - (u32)c0);
    if (span <= 0) return false;
    float fr = (float)distance / (float)span;
    fr = fr > 0.0f ? fr : 0.0f;
    fr = fr < 1.0f ? fr : 1.0f;
    out = fr * (high - low) + low;
    return finite(out);
}
HD u32 item_key(const G& g, const Cand& q, i32 bank, i32 slot) {
    if (bank >= 0 && bank < 2 && slot >= 0 && slot < 17) return u16_at(item_ptr(g, q, (u32)bank, (u32)slot), 0);
    return u16_at(g.c.inv, 0);
}
HD void add_u32(u8* b, u32 at, u32 by) { put_u32(b, at, u32_at(b, at) + by); }
HD bool add_f32(u8* b, u32 at, float by) {
    float old = f32_at(b, at);
    if (!finite(old)) return false;
    float v = by + old;
    if (!finite(v)) return false;
    put_f32(b, at, v);
    return true;
}
__constant__ u32 ACTIVE_CATEGORY[16] = {0x5197, 0x78F4, 0xC811, 0x9002, 0xBABC, 0xCC71, 0x437D, 0x6670, 0xA335, 0x36F4, 0x5494, 0x4674, 0x0AC0, 0xC86A, 0xB93B, 0x94E2};
__constant__ u32 FIXED_CATEGORY[11][2] = {{0xF5E5, 0x1909}, {0x6449, 0x6E73}, {0x20FF, 0x5095}, {0x4420, 0x1C17}, {0x7F23, 0x2D3F}, {0xD081, 0x72B1},
                                          {0x04BE, 0x5E1B}, {0x5F3A, 0x027C}, {0x1AE7, 0x0D2F}, {0x763B, 0x17D6}, {0xAF69, 0x04E6}};
HD bool selected_slot(const G& g, i32 bank, i32& out) {
    if (bank < 0 || bank >= 2) return false;
    out = g.c.inv[(u32)bank * BANK + 0xF68] == 0 ? 1 : 0;
    return true;
}
HD bool active_category(u32 v) { for (int i = 0; i < 16; i++) if (ACTIVE_CATEGORY[i] == v) return true; return false; }
HD bool category_of(const G& g, u32 key, u32& out) { const u8* r = k_row(g.items, key); if (!r) return false; out = u32_at(r, 0x58); return true; }
HD bool materialize_one(const G& g, const Cand& q, u32 group, u32 v, S& s) {
    Klass k = g.klass[group];
    u8* stat = s.stat; u8* d = s.d;
    const i32* attrs = q.attrs;
    i32 bank = i32_at(g.c.inv, 0x1EE0);
    if (k.kind == 1) return true;
    if (k.kind == 2) {
        for (u32 i = k.start; i < k.start + k.n; i++) {
            Recipe r = g.recipes[i];
            u32 at = r.offset;
            switch (r.kind) {
            case 0: for (u32 b = 0; b < r.width; b++) d[at + b] = (u8)(r.literal >> (8 * b)); break;
            case 1: case 2: {
                float value = (float)v;
                if (r.has_scale) value *= r.scale;
                if (r.kind == 2) { float old = f32_at(d, at); if (!finite(old)) return false; value += old; }
                if (!finite(value)) return false;
                put_f32(d, at, value);
                break;
            }
            case 3: { u32 old = u32_at(d, at); put_u32(d, at, old + (u32)imax((i32)old, (i32)v)); break; }
            case 4: { u32 old = u32_at(d, at); put_u32(d, at, (u32)imax((i32)old, (i32)v)); break; }
            case 5: add_u32(d, at, v); break;
            case 6: { float term = (float)v; if (r.has_scale) term *= r.scale; TRY(add_f32(stat, at, term)); break; }
            case 7: put_u16(stat, at, u16_at(stat, at) + (v & 65535)); break;
            case 8: add_u32(stat, at, v); break;
            default: return false;
            }
        }
        return true;
    }
    if (k.kind == 3) {
        float x;
        switch (group) {
        case 0x8C47: if (stat[0x61] == 0) add_u32(stat, 0x2C, v); if (stat[0xA1] == 0) add_u32(stat, 0x6C, v); return true;
        case 0x8703: if (stat[0x161] == 0 && u32_at(stat, 0x124) != 0) add_u32(stat, 0x12C, v); return true;
        case 0xDCBD: case 0xB00A: case 0x39B8: case 0x24EE: case 0xE5B9: case 0x09B1: case 0x8997: {
            int index = group == 0xDCBD ? 0 : group == 0xB00A ? 1 : group == 0x39B8 ? 2 : group == 0x24EE ? 3 : group == 0xE5B9 ? 4 : group == 0x09B1 ? 5 : 6;
            TRY(grade_of(g, 0x6498, attrs[index], v, x));
            add_u32(stat, 0x1C8, (u32)cvtt64(x));
            return true;
        }
        case 0x3503: case 0x684F: case 0xBB6D: case 0x794D: case 0xEEEF: case 0xA71E: {
            u32 tag = group == 0x3503 ? 0 : group == 0x684F ? 1 : group == 0xBB6D ? 2 : group == 0x794D ? 22 : group == 0xEEEF ? 3 : 4;
            i32 slot;
            TRY(selected_slot(g, bank, slot));
            u32 at = 0x3C + 64 * (u32)slot;
            if (u32_at(stat, at) == ABSENT) { put_u32(stat, at, tag); put_u32(stat, at + 4, v); }
            return true;
        }
        case 0x3000: case 0xB2FF: {
            i32 empty = 0;
            for (int i = 0; i < 8; i++) if (item_key(g, q, bank, g.slots8[i]) == 0) empty++;
            TRY(grade_of(g, group == 0x3000 ? 0x8AF7 : 0xA9A, empty, v, x));
            return add_f32(d, group == 0x3000 ? 0x108 : 0x294, x * g.c_percent);
        }
        case 0x2C9E: case 0x2E5B:
            TRY(grade_of(g, group == 0x2C9E ? 0xBE72 : 0xF919, attrs[group == 0x2C9E ? 6 : 4], v, x));
            return add_f32(d, group == 0x2C9E ? 0x684 : 0x794, x * g.c_percent);
        case 0x5D3A: {
            TRY(grade_of(g, 0xFA3E, 1, v, x));
            float factor = x * g.c_percent;
            add_u32(stat, 0x18, (u32)cvtt64(factor * (float)field90(g, attrs[2], 0xC)));
            return true;
        }
        case 0xD84B: {
            float old = f32_at(stat, 0x55C);
            if (!finite(old)) return false;
            TRY(grade_of(g, 0xA7C5, (i32)cvtt64(old), v, x));
            return add_f32(d, 0xCC, x * g.c_percent - g.c_one);
        }
        case 0xDFC6: {
            float old = f32_at(stat, 0x7B0);
            if (!finite(old)) return false;
            float value = old - (float)v * g.c_4688;
            if (!finite(value)) return false;
            put_f32(stat, 0x7B0, g.opt_d1f <= value ? value : g.opt_d1f);
            return true;
        }
        case 0xB3FE: {
            u32 old = u32_at(stat, 0x7B4);
            float term = (float)old * (float)v * g.c_4688;
            put_u32(stat, 0x7B4, (u32)((i64)old + cvtt64(term)));
            return true;
        }
        case 0xB148: {
            float denominator = g.opt_22af * g.opt_1f32 * g.c_4080;
            float old = f32_at(stat, 0x7A4);
            if (!finite(old)) return false;
            float acc = (float)v / denominator + old;
            if (!finite(acc)) return false;
            put_f32(stat, 0x7A4, acc);
            float next = g.opt_1f32 * g.opt_22af * acc;
            if (!finite(next)) return false;
            put_f32(stat, 0x7A8, next);
            return true;
        }
        case 0xCBEE: case 0xE63F: {
            i32 count = 0;
            for (int i = 0; i < 5; i++) { const u8* r = k_row(g.items, item_key(g, q, bank, g.slots5[i])); if (r && (u32_at(r, 0xB0) & 0x100)) count++; }
            if (group == 0xCBEE) return add_f32(d, 0xCC, (float)v * g.c_4688 * (float)count);
            if (count >= 5) add_u32(d, 0x61C, 1);
            return true;
        }
        default:
            if (active_category(group)) {
                i32 slot;
                TRY(selected_slot(g, bank, slot));
                const u8* row = k_row(g.groups, group);
                if (!row) return false;
                u32 target = u32_at(row, 0x20), key = item_key(g, q, bank, slot), category = 0x3C2E;
                if (key != 0) category_of(g, key, category);
                if (category == target) {
                    TRY(grade_of(g, 0x1DF9, 0, v, x));
                    TRY(add_f32(d, 0xCC, x * g.c_percent));
                    TRY(grade_of(g, 0x874A, 0, v, x));
                    TRY(add_f32(d, 0xD0, x * g.c_percent));
                }
                return true;
            }
            return false;
        }
    }
    if (k.kind == 4) {
        if (group == 0xF919 || group == 0x052D) {
            if (g.c.mode == (group == 0x052D ? 1 : 0)) { add_u32(stat, 0x2C, v); add_u32(stat, 0x6C, v); }
            return true;
        }
        int fixed = -1;
        for (int i = 0; i < 11; i++) if (FIXED_CATEGORY[i][0] == group) { fixed = i; break; }
        u32 target;
        if (fixed >= 0) target = FIXED_CATEGORY[fixed][1];
        else { const u8* row = k_row(g.groups, group); if (!row) return false; target = u32_at(row, 0x20); }
        for (u32 slot = 0; slot < 2; slot++) {
            u32 key = item_key(g, q, bank, (i32)slot), category = 0;
            bool some = category_of(g, key, category);
            if (fixed < 0 && (key == 0 || !some)) { category = 0x3C2E; some = true; }
            if (some && category == target) add_u32(stat, 0x2C + 0x40 * slot, v);
        }
        return true;
    }
    return false;
}
HD bool start(const G& g, const Cand& q, S& s) {
    const u32* from = (const u32*)g.stat; u32* to = (u32*)s.stat;
    for (u32 i = 0; i < g.stat_live_n; i++) to[g.stat_live[i]] = from[g.stat_live[i]];
    from = (const u32*)g.d0; to = (u32*)s.d;
    for (u32 i = 0; i < g.d_live_n; i++) to[g.d_live[i]] = from[g.d_live[i]];
    for (u32 i = 0; i < 5; i++) put_u64(s.stat, 0x24 + 64 * i, 0);
    put_u64(s.stat, 8, 0);
    put_u64(s.stat, 0x7D8, g.attr);
    s.stat[0x7D0] = 0;
    s.mode = g.c.mode;
    for (u32 i = 0; i < 7; i++) {
        s.thresholds[i] = (u32)q.attrs[i];
        u32 index = (u32)clamp999(q.attrs[i]);
        if (index >= u32_at(g.base90, 4)) return false;
        s.transformed[i] = (i32)u16_at(g.base90, 8 + 32 * index + 0x1A);
    }
    s.map_n[0] = s.map_n[1] = s.side_n[0] = s.side_n[1] = 0;
    return prepared(g, q, s);
}
// What the weapons leave behind is the same for every outfit of a space, so it is computed once and copied.
HD void resume(const G& g, S& s) {
    const S& from = *g.seed;
    const u32* a = (const u32*)from.stat; u32* b = (u32*)s.stat;
    for (u32 i = 0; i < g.stat_live_n; i++) b[g.stat_live[i]] = a[g.stat_live[i]];
    a = (const u32*)from.d; b = (u32*)s.d;
    for (u32 i = 0; i < g.d_live_n; i++) b[g.d_live[i]] = a[g.d_live[i]];
    s.stat[8] = from.stat[8]; put_u64(s.stat, 8, 0); put_u64(s.stat, 0x7D8, g.attr); s.stat[0x7D0] = 0;
    s.mode = from.mode;
    for (u32 i = 0; i < 7; i++) { s.thresholds[i] = from.thresholds[i]; s.transformed[i] = from.transformed[i]; }
    for (u32 m = 0; m < 2; m++) {
        s.map_n[m] = from.map_n[m]; s.side_n[m] = from.side_n[m];
        for (u32 i = 0; i < from.map_n[m]; i++) s.maps[m][i] = from.maps[m][i];
        for (u32 i = 0; i < from.side_n[m]; i++) for (u32 w = 0; w < 17; w++) ((u32*)s.sides[m][i])[w] = ((const u32*)from.sides[m][i])[w];
    }
}
HD bool pipeline(const G& g, const Cand& q, S& s) {
    if (g.seed) resume(g, s);
    else TRY(start(g, q, s));
    TRY(body(g, q, s));
    if (u16_at(g.c.inv, g.c.active * BANK + 15 * ITEM) != 0) TRY(slot15(g, s));
    TRY(set_rewards(g, s));
    int bank = selected_map(g.c.mode, g.c.mode);
    Entry* entries = s.maps[bank];
    u32 n = s.map_n[bank];
    for (u32 o = 0; o < g.op_n; o++) {
        u32 group = g.op_group[o];
        i64 value = g.op_value[o];
        u32 i = 0;
        while (i < n && entries[i].key != group) i++;
        if (i < n) {
            entries[i].base += (u32)value;
            if (value < 0 && entries[i].base == 0) { for (u32 j = i + 1; j < n; j++) entries[j - 1] = entries[j]; n--; }
        } else {
            if (n >= 300) return false;
            entries[n].key = (u16)group; entries[n].base = (u32)value; entries[n].dup = 0; n++;
        }
    }
    s.map_n[bank] = n;
    if (n > 256) return false;
    u32 capacity = field90(g, q.attrs[2], g.c.mode == 0 ? 8 : 10);
    float ratio = (float)u32_at(s.stat, 0x7B8) / (float)capacity;
    put_f32(s.stat, 0x7A0, g.opt_d84f + ratio * g.opt_dc22);
    put_u64(s.stat, 8, g.owner);
    put_u64(s.stat, 0x7C0, g.dest);
    for (u32 i = 0; i < n; i++) { Entry e = entries[i]; TRY(materialize_one(g, q, e.key, e.base + e.dup, s)); }
    return true;
}
HD void write_at(const G& g, S& s, u32 w, u32 value) {
    u32 m = g.word_mask[w];
    s.dom[w] = (s.dom[w] & ~m) | (value & m);
    s.known[w] |= m;
}
HD void write_word(const G& g, S& s, u64 address, u32 value) {
    for (int i = 0; i < g.ctx.domains; i++) {
        Span d = g.ctx.domain[i];
        if (d.s <= address && address < d.e) {
            u32 slim = g.ctx.slim_of[(u32)(d.o + address - d.s) / 4];
            if (slim != 0xffff) write_at(g, s, slim, value);
            return;
        }
    }
}
HD bool frame_writes(const G& g, const Cand& q, S& s) {
    u8* pred = s.pred;
    const u32* base_w = (const u32*)g.s_base; const u32* stat_w = (const u32*)s.stat; u32* pred_w = (u32*)pred;
    for (u32 i = 0; i < g.pred_live_n; i++) {
        u32 w = g.pred_live[i];
        pred_w[w] = ((const u32*)g.stat)[w];
        if (w == 2 || w == 3 || w >= 0x7A0 / 4) continue;
        u32 ia = base_w[w], ib = stat_w[w], ic = pred_w[w];
        if (ia == ib) continue;
        if (umax(umax(ia, ib), ic) < (1u << 24)) pred_w[w] = ic + ib - ia;
        else {
            double v = (double)bits_f(ic) + (double)bits_f(ib) - (double)bits_f(ia);
            float narrow = (float)v;
            if (fabs(v) <= 1.797693134862315708e308 && !finite(narrow) ) { union { u32 u; float x; } c; c.x = narrow; if ((c.u & 0x7fffffffu) == 0x7f800000u) return false; }
            pred_w[w] = f_bits(narrow);
        }
    }
    int bank = selected_map(g.c.mode, g.c.mode);
    u64 atk = 0;
    for (u32 i = 0; i < s.map_n[bank]; i++) if (s.maps[bank][i].key == 0x6A7C) { atk = (u64)s.maps[bank][i].base + s.maps[bank][i].dup; break; }
    put_u32(pred, 0x7A0, u32_at(s.stat, 0x7A0));
    put_u32(pred, 0x7B8, u32_at(s.stat, 0x7B8));
    for (u32 i = 0; i < 5; i++) {
        u64 v = (u64)u32_at(s.stat, 0x24 + 64 * i) + g.gain;
        u64 plus8 = atk + (i < 2 ? PLUS8_WEAPON : 0);
        if (v > 0xffffffffull || plus8 > 0xffffffffull) return false;
        put_u32(pred, 0x24 + 64 * i, (u32)v);
        put_u32(pred, 0x28 + 64 * i, (u32)v);
        put_u32(pred, 0x2C + 64 * i, (u32)plus8);
    }
    put_u32(pred, 0x12C, atk > 0xffffffffull ? 0xffffffffu : (u32)atk);
    const i32* attrs = q.attrs;
    put_u32(pred, 0x7B4, field90(g, attrs[2], g.c.mode == 0 ? 8 : 0xA));
    put_u32(pred, 0x1BC, field90(g, attrs[4], 0x10));
    put_u32(pred, 0x1C0, field90(g, attrs[4], 0x12));
    put_u32(pred, 0x1C4, field90(g, attrs[6], 0x18) + 11);
    float scale = g.c_4894;
    float recovery = (float)field90(g, attrs[5], 0x16) * scale + (float)field90(g, attrs[1], 6) * scale;
    put_f32(pred, 0x7A4, recovery);
    put_f32(pred, 0x7A8, g.opt_recovery * recovery);
    put_f32(pred, 0x1A8, (float)field90(g, attrs[3], 0xE) * scale + g.c_one);
    put_f32(pred, 0x1AC, (float)field90(g, attrs[5], 0x14) * scale + g.c_one);
    put_f32(pred, 0x7B0, g.opt_e40f);
    i64 hp = (i64)(field90(g, (i32)g.c.level, 0) + field90(g, attrs[0], 2) + field90(g, attrs[2], 0xC));
    i64 health = (i64)u32_at(pred, 0x18) + hp - (i64)g.hp_base;
    i64 armor = (i64)u32_at(pred, 0x1C8) - BASE_DEFENSE_SOURCES + (i64)g.defense;
    if (health < 0 || health >= (1ll << 32) || armor < 0 || armor >= (1ll << 32)) return false;
    put_u32(pred, 0x18, (u32)health);
    put_u32(pred, 0x1C8, (u32)armor);
    const u32* dom0 = (const u32*)g.dom_base; const u32* known0 = (const u32*)g.dom_known;
    for (u32 i = 0; i < SLIM_WORDS; i++) { s.dom[i] = dom0[i]; s.known[i] = known0[i]; }
    const u32* frame = (const u32*)g.stat;
    for (u32 i = 0; i < g.stat_out_n; i++) { u32 w = g.stat_out[2 * i]; if (pred_w[w] != frame[w]) write_at(g, s, g.stat_out[2 * i + 1], pred_w[w]); }
    const u32* now = (const u32*)s.d; const u32* zero = (const u32*)g.d0; const u32* based = (const u32*)g.d_base;
    for (u32 i = 0; i < g.d_out_n; i++) { u32 w = g.d_out[2 * i]; if (now[w] != zero[w] || based[w] != zero[w]) write_at(g, s, g.d_out[2 * i + 1], now[w]); }
    u32 packet = g.main8 + (u32_at(pred, 0x24) + u32_at(pred, 0x2C)) - (u32_at(g.stat, 0x24) + u32_at(g.stat, 0x2C));
    write_word(g, s, g.main_addr + 8, packet);
    write_word(g, s, g.main_addr + 0xC, u32_at(pred, 0x30));
    write_word(g, s, g.main_addr + 0x10, u32_at(pred, 0x34));
    write_word(g, s, g.main_addr + 0x14, u32_at(pred, 0x3C));
    write_word(g, s, g.ext_addr + 0x7B8, u32_at(pred, 0x7B8));
    for (u32 i = 0; i < 7; i++) write_word(g, s, g.stat_addr + 0x844 + 4 * i, (u32)attrs[i]);
    return true;
}
HD float load_of(const G& g, const Cand& q, const S& s) { return (float)u32_at(s.stat, 0x7B8) / (float)field90(g, q.attrs[2], g.c.mode == 0 ? 8 : 10); }
// 0 refused by the pipeline, 1 answered, 2 no compiled path, 3 outside the player's limits
HD int evaluate(const G& g, const Cand& q, S& s, u32& bits) {
    if (!pipeline(g, q, s) || !frame_writes(g, q, s)) return 0;
    if (g.limits.on && (load_of(g, q, s) > g.limits.max_load || u32_at(s.pred, 0x1C8) < g.limits.min_defense || u32_at(s.pred, 0x18) < g.limits.min_health)) return 3;
    Ctx c = g.ctx;
    c.dom = (const u8*)s.dom;
    c.known = (const u8*)s.known;
    return run_main(c, bits) ? 1 : 2;
}
__global__ void kernel_cases(G g, const Cand* cases, u64 n, u32* out, u8* ok) {
    u64 i = blockIdx.x * (u64)blockDim.x + threadIdx.x;
    if (i >= n) return;
    S s;
    Cand q = cases[i];
    u32 bits = 0;
    ok[i] = (u8)evaluate(g, q, s, bits);
    out[i] = bits;
}
__global__ void kernel_seed(G g, Cand q, S* out, u32* ok) {
    if (blockIdx.x != 0 || threadIdx.x != 0) return;
    *ok = start(g, q, *out) ? 1 : 0;
}
__global__ void kernel_facts(G g, const Cand* cases, u64 n, float* load, u32* defense, u32* health) {
    u64 i = blockIdx.x * (u64)blockDim.x + threadIdx.x;
    if (i >= n) return;
    S s;
    Cand q = cases[i];
    load[i] = 0.0f; defense[i] = health[i] = 0;
    if (pipeline(g, q, s) && frame_writes(g, q, s)) { load[i] = load_of(g, q, s); defense[i] = u32_at(s.pred, 0x1C8); health[i] = u32_at(s.pred, 0x18); }
}
__global__ void kernel_dump(G g, const Cand* cases, u64 n, u32* out) {
    u64 i = blockIdx.x * (u64)blockDim.x + threadIdx.x;
    if (i >= n) return;
    S s;
    Cand q = cases[i];
    u32 bits = 0;
    evaluate(g, q, s, bits);
    for (u32 w = 0; w < SLIM_WORDS; w++) out[i * (SLIM_WORDS + 1) + w] = s.dom[w];
    out[i * (SLIM_WORDS + 1) + SLIM_WORDS] = bits;
}
__global__ void kernel_search(G g, Space sp, u64 first, u64 total, u32* best, Hit* hits, u32* hit_n, u32 capacity, u32* counts, u64* missed, u32 miss_capacity) {
    u64 i = first + blockIdx.x * (u64)blockDim.x + threadIdx.x;
    if (i >= total) return;
    S s;
    Cand q;
    for (int a = 0; a < 7; a++) q.attrs[a] = sp.attrs[a];
    for (int a = 0; a < CAND_SLOTS; a++) q.slot[a] = -1;
    u64 digits = i;
    for (int a = 0; a < sp.n; a++) { q.slot[cand_index((u32)sp.slot[a])] = (i32)(sp.start[a] + (u32)(digits % sp.radix[a])); digits /= sp.radix[a]; }
    u32 bits = 0;
#if PROFILE
    if ((i & 255) == 0 && pipeline(g, q, s) && frame_writes(g, q, s)) {
        const u32* dom0 = (const u32*)g.dom_base; const u32* known0 = (const u32*)g.dom_known;
        for (u32 w = 0; w < SLIM_WORDS; w++) { atomicOr(g.ctx.prof + 2 * PROF_N + 2 * w, s.dom[w] ^ dom0[w]); atomicOr(g.ctx.prof + 2 * PROF_N + 2 * w + 1, s.known[w] ^ known0[w]); }
    }
#endif
    if (sp.pipeline_only) { bits = pipeline(g, q, s) && frame_writes(g, q, s) ? s.dom[8] : 0; atomicAdd(&counts[1], bits ? 1u : 1u); return; }
    int r = evaluate(g, q, s, bits);
    if (r != 1) { u32 at = atomicAdd(&counts[r], 1u); if (r == 2 && at < miss_capacity) missed[at] = i | (u64)sp.id << 48; return; }
    if (!(bits_f(bits) > 0.0f)) return;
    u32 seen = atomicMax(best, bits);
    if (bits_f(bits) >= bits_f(seen) * sp.keep) {
        u32 at = atomicAdd(hit_n, 1u);
        if (at < capacity) { hits[at].bits = bits; hits[at].space = (u32)sp.id; hits[at].index = i; }
    }
}
#define CHECK(x) do { cudaError_t e = (x); if (e != cudaSuccess) { printf("%s:%d %s\n", __FILE__, __LINE__, cudaGetErrorString(e)); exit(1); } } while (0)
typedef std::vector<u8> Bytes;
static Bytes slurp(const std::string& path) { FILE* f = fopen(path.c_str(), "rb"); if (!f) { printf("cannot read %s\n", path.c_str()); exit(1); } fseek(f, 0, SEEK_END); long n = ftell(f); fseek(f, 0, SEEK_SET); Bytes b(n); if (n) fread(b.data(), 1, n, f); fclose(f); return b; }
template <class T> static T* up(const T* src, size_t n) { T* p; CHECK(cudaMalloc(&p, (n ? n : 1) * sizeof(T))); if (n) CHECK(cudaMemcpy(p, src, n * sizeof(T), cudaMemcpyHostToDevice)); return p; }
typedef std::map<std::string, Bytes> Sections;
static u32 h32(const u8* p) { return *(const u32*)p; }
static Sections sections(const std::string& path) {
    Bytes d = slurp(path); Sections out; size_t at = 0;
    while (at < d.size()) { u32 n = h32(&d[at]); std::string name((const char*)&d[at + 4], n); at += 4 + n; u32 size = h32(&d[at]); out[name] = Bytes(d.begin() + at + 4, d.begin() + at + 4 + size); at += 4 + size; }
    return out;
}
static std::vector<u32> words(const Bytes& b) { std::vector<u32> w(b.size() / 4); if (!w.empty()) memcpy(w.data(), b.data(), w.size() * 4); return w; }
static Keyed keyed(Sections& s, const char* name, u32 stride) {
    Bytes& rows = s[std::string(name) + ".rows"];
    std::vector<u32> w = words(s[std::string(name) + ".keys"]);
    std::vector<std::pair<u32, u32>> pairs;
    for (size_t i = 0; i + 1 < w.size(); i += 2) pairs.push_back({w[i], w[i + 1]});
    std::sort(pairs.begin(), pairs.end());
    Keyed k{}; k.rows = up(rows.data(), rows.size()); k.size = (u32)rows.size(); k.stride = stride;
    if (pairs.empty() || pairs.back().first < (1u << 20)) {
        std::vector<u32> direct(pairs.empty() ? 0 : pairs.back().first + 1, ABSENT);
        for (auto& p : pairs) direct[p.first] = p.second;
        k.direct = up(direct.data(), direct.size()); k.direct_n = (u32)direct.size();
    } else {
        std::vector<u32> flat; for (auto& p : pairs) { flat.push_back(p.first); flat.push_back(p.second); }
        k.sparse = up(flat.data(), flat.size()); k.sparse_n = (u32)pairs.size();
    }
    return k;
}
struct Scored { u32 bits; Cand q; };
// Pruning is allowed to be wrong in small ways (a heavier item beats its "dominator" by a point), so the best outfits get every pruned item back,
// one slot at a time, until nothing improves. gpu_full.txt: pruned_start pruned_count full_start full_count, for each pruned list.
static void polish(const G& g, const std::string& dir, std::vector<Scored>& top, bool move_points) {
    struct Link { u32 start, count, full, full_n; };
    std::vector<Link> links;
    FILE* f = fopen((dir + "/gpu_full.txt").c_str(), "r");
    if (f) {
        for (Link l; fscanf(f, "%u %u %u %u", &l.start, &l.count, &l.full, &l.full_n) == 4;) links.push_back(l);
        fclose(f);
    }
    const u64 block = 128;
    for (int round = 0; round < 40; round++) {
        std::vector<Cand> tries;
        if (move_points) for (size_t r = 0; r < top.size() && r < 40; r++) for (int from = 0; from < 7; from++) for (int to = 0; to < 7; to++) for (int step : {1, 3, 10}) {
            if (from == to || top[r].q.attrs[from] - step < 5) continue;
            Cand c = top[r].q; c.attrs[from] -= step; c.attrs[to] += step; tries.push_back(c);
        }
        for (const Scored& row : top) for (int a = 0; a < CAND_SLOTS; a++) {
            i32 held = row.q.slot[a];
            if (held < 0) continue;
            for (const Link& l : links) {
                if ((u32)held < l.start || (u32)held >= l.start + l.count) continue;
                for (u32 k = 0; k < l.full_n; k++) { Cand c = row.q; c.slot[a] = (i32)(l.full + k); tries.push_back(c); }
            }
        }
        if (tries.empty()) return;
        std::sort(tries.begin(), tries.end(), [](const Cand& a, const Cand& b) { return memcmp(&a, &b, sizeof(Cand)) < 0; });
        tries.erase(std::unique(tries.begin(), tries.end(), [](const Cand& a, const Cand& b) { return memcmp(&a, &b, sizeof(Cand)) == 0; }), tries.end());
        u64 n = tries.size();
        const Cand* dc = up(tries.data(), n);
        u32* out; u8* ok;
        CHECK(cudaMalloc(&out, 4 * n)); CHECK(cudaMalloc(&ok, n));
        kernel_cases<<<(unsigned)((n + block - 1) / block), block>>>(g, dc, n, out, ok);
        CHECK(cudaDeviceSynchronize());
        std::vector<u32> got(n); std::vector<u8> fine(n);
        CHECK(cudaMemcpy(got.data(), out, 4 * n, cudaMemcpyDeviceToHost)); CHECK(cudaMemcpy(fine.data(), ok, n, cudaMemcpyDeviceToHost));
        CHECK(cudaFree((void*)dc)); CHECK(cudaFree(out)); CHECK(cudaFree(ok));
        float before = top.empty() ? 0.0f : bits_f(top[0].bits);
        std::vector<Scored> merged = top;
        for (u64 i = 0; i < n; i++) if (fine[i] == 1) merged.push_back(Scored{got[i], tries[i]});
        std::stable_sort(merged.begin(), merged.end(), [](const Scored& a, const Scored& b) { return bits_f(a.bits) > bits_f(b.bits); });
        merged.erase(std::unique(merged.begin(), merged.end(), [](const Scored& a, const Scored& b) { return memcmp(&a.q, &b.q, sizeof(Cand)) == 0; }), merged.end());
        if (merged.size() > 400) merged.resize(400);
        top = merged;
        printf("polish round %d: %llu tried, best %.3f -> %.3f\n", round, (unsigned long long)n, before, bits_f(top[0].bits));
        if (!(bits_f(top[0].bits) > before)) return;
    }
}
int main(int argc, char** argv) {
    std::string dir = argv[1];
    float keep = argc > 2 ? (float)atof(argv[2]) : 0.97f;
    Sections s = sections(dir + "/tables.bin");
    G g{};
    g.items = keyed(s, "item", 0x1A0); g.effects = keyed(s, "effect", 0xD8); g.groups = keyed(s, "group", 0x70); g.rarity = keyed(s, "rarity", 0xF8);
    g.sides = keyed(s, "side", 0xD8); g.special = keyed(s, "special", 0x1C); g.core300 = keyed(s, "core300", 0x80); g.core340 = keyed(s, "core340", 0x44);
    g.level_rows = up(s["level"].data(), s["level"].size()); g.base70 = up(s["base70"].data(), s["base70"].size()); g.base70_rows = (u32)(s["base70"].size() / 0x94);
    g.power90 = up(s["power90"].data(), s["power90"].size()); g.base90 = up(s["base90"].data(), s["base90"].size()); g.grade_rows = up(s["grade.rows"].data(), s["grade.rows"].size());
    std::vector<u32> fam = words(s["fam"]), alt = words(s["alternate"]), c = words(s["consts"]), gw = words(s["grade.index"]), m = words(s["materializer"]);
    g.fam_steps = fam[0]; g.fam_bonus_steps = fam[1]; g.fam = up(fam.data() + 2, fam.size() - 2);
    g.alternate_default = alt[0]; g.alternate_n = (u32)((alt.size() - 1) / 2); g.alternate = up(alt.data() + 1, alt.size() - 1);
    std::vector<u32> gi(131072, 0);
    for (size_t i = 0; i + 2 < gw.size(); i += 3) { gi[2 * gw[i]] = gw[i + 1]; gi[2 * gw[i] + 1] = gw[i + 2]; }
    g.grade_index = up(gi.data(), gi.size());
    std::vector<Klass> klass(65536, Klass{0, 0, 0}); std::vector<Recipe> recipes;
    for (size_t at = 0; at < m.size();) {
        u32 group = m[at], kind = m[at + 1], n = m[at + 2]; at += 3;
        klass[group] = Klass{kind, (u32)recipes.size(), n};
        for (u32 i = 0; i < n; i++, at += 7) { const u32* r = &m[at]; union { u32 u; float x; } f; f.u = r[4]; recipes.push_back(Recipe{r[0], r[1], r[2], r[3], f.x, 0, (u64)r[5] | (u64)r[6] << 32}); }
    }
    memcpy(g.families, s["families"].data(), 8); memcpy(g.slots5, s["slots5"].data(), 20); memcpy(g.slots8, s["slots8"].data(), 32);
    float* cf = (float*)c.data();
    g.c_percent = cf[0]; g.c_4688 = cf[1]; g.c_one = cf[2]; g.c_half = cf[3]; g.c_tenth = cf[4]; g.c_atten = cf[5]; g.c_4894 = cf[6]; g.c_4080 = cf[7]; g.opt_recovery = cf[8]; g.opt_e40f = cf[9];
    g.opt_22af = cf[10]; g.opt_1f32 = cf[11]; g.opt_d1f = cf[12]; g.opt_dc22 = cf[13]; g.opt_d84f = cf[14]; g.aeac_int = (i32)c[15];
    Sections fr = sections(dir + "/frame.bin");
    const u64* q = (const u64*)fr["addresses"].data();
    g.stat = up(fr["stat"].data(), 0x900); g.d0 = up(fr["d0"].data(), 0xB00); g.s_base = up(fr["s_base"].data(), fr["s_base"].size()); g.d_base = up(fr["d_base"].data(), 0xB00);
    g.stat_addr = q[0]; g.d_addr = q[1]; g.main_addr = q[2]; g.ext_addr = q[3]; g.owner = q[4]; g.attr = q[5]; g.dest = q[6]; g.main8 = (u32)q[7]; g.hp_base = (u32)q[8];
    Sections ch = sections(dir + "/characters.bin");
    Bytes& cb = ch["char0"];
    const u32* w = (const u32*)cb.data();
    g.c.mode = (i32)w[0]; g.c.active = w[1]; g.c.cap = w[2]; g.c.plus_cap = w[3]; g.c.style = (i32)w[4]; g.c.unarmed = w[5]; g.c.level = w[6]; g.c.core_mode = (i32)w[7];
    g.c.core_n[0] = w[8]; g.c.core_n[1] = w[9];
    g.c.inv = up(cb.data() + 40, 0x2018);
    std::vector<Entry> ce;
    for (size_t at = 40 + 0x2018; at + 8 <= cb.size(); at += 8) { Entry e; e.key = *(u16*)&cb[at]; e.dup = *(u16*)&cb[at + 2]; e.base = h32(&cb[at + 4]); ce.push_back(e); }
    const Entry* ced = up(ce.data(), ce.size());
    g.c.core[0] = ced; g.c.core[1] = ced + w[8];
    Bytes pool = slurp(dir + "/pool.bin"), arena = slurp(dir + "/arena.bin"), dom = slurp(dir + "/dom_base.bin"), known = slurp(dir + "/dom_known.bin"), mapped = slurp(dir + "/mapped.bin");
    g.pool = up(pool.data(), pool.size()); g.dom_size = (u32)dom.size();
    u32 full_words = (u32)(dom.size() / 4);
    std::vector<u16> slim_of(full_words, 0xffff);
    std::vector<u32> masks(SLIM_WORDS, 0), dom0(SLIM_WORDS), known0(SLIM_WORDS);
    for (u32 i = 0; i < SLIM_WORDS; i++) {
        u32 w = SLIM_LIST[i];
        slim_of[w] = (u16)i;
        for (u32 b = 0; b < 4; b++) if (mapped[4 * w + b]) masks[i] |= 0xffu << (8 * b);
        dom0[i] = h32(&dom[4 * w]); known0[i] = h32(&known[4 * w]);
    }
    g.word_mask = up(masks.data(), masks.size()); g.dom_base = (const u8*)up(dom0.data(), dom0.size()); g.dom_known = (const u8*)up(known0.data(), known0.size());
    // Words of STAT and D the formula can see, and the ones the pipeline itself reads back. Everything else is never touched, so a candidate stays small in cache.
    std::vector<u32> stat_out, d_out, stat_live, d_live, pred_live;
    std::vector<char> in_stat(0x900 / 4, 0), in_d(0xB00 / 4, 0), in_pred(0x900 / 4, 0);
    for (const Span& d : DOM_SPANS) {
        if (d.o % 4 || (d.e - d.s) % 4) { printf("domain span not word aligned\n"); return 1; }
        for (u64 a = d.s; a < d.e; a += 4) {
            u32 slim = slim_of[(u32)(d.o + a - d.s) / 4];
            if (slim == 0xffff) continue;
            if (a >= g.stat_addr && a < g.stat_addr + 0x7C0) { u32 w = (u32)(a - g.stat_addr) / 4; if (w != 2 && w != 3) { stat_out.push_back(w); stat_out.push_back(slim); in_stat[w] = in_pred[w] = 1; } }
            if (a >= g.d_addr && a < g.d_addr + 0xB00) { u32 w = (u32)(a - g.d_addr) / 4; d_out.push_back(w); d_out.push_back(slim); in_d[w] = 1; }
        }
    }
    for (u32 w = 0x24 / 4; w < 0x1A4 / 4; w++) in_stat[w] = 1;
    for (u32 at : {0x18u, 0x1C8u, 0x218u, 0x21Cu, 0x7A0u, 0x7A4u, 0x7A8u, 0x7B0u, 0x7B4u, 0x7B8u, 0x55Cu}) in_stat[at / 4] = 1;
    for (u32 at : {0xCCu, 0xD0u, 0x108u, 0x294u, 0x684u, 0x794u, 0x61Cu}) in_d[at / 4] = 1;
    for (u32 at : {0x18u, 0x1C8u, 0x24u, 0x28u, 0x2Cu, 0x30u, 0x34u, 0x3Cu, 0x7B8u}) in_pred[at / 4] = 1;
    for (u32 i = 0; i < 5; i++) for (u32 at : {0x24u, 0x28u, 0x2Cu}) in_pred[(at + 64 * i) / 4] = 1;
    for (u32 w = 0; w < in_stat.size(); w++) { if (in_stat[w]) stat_live.push_back(w); if (in_pred[w]) pred_live.push_back(w); }
    for (u32 w = 0; w < in_d.size(); w++) if (in_d[w]) d_live.push_back(w);
    static const u32 body_words[12] = {0x77c, 0x780, 0x784, 0x788, 0x78c, 0x790, 0x4dc, 0x4fc, 0x51c, 0x53c, 0x55c, 0x57c};
    for (int i = 0; i < 12; i++) g.body_live[i] = in_stat[body_words[i] / 4];
    g.stat_live = up(stat_live.data(), stat_live.size()); g.stat_live_n = (u32)stat_live.size(); g.d_live = up(d_live.data(), d_live.size()); g.d_live_n = (u32)d_live.size();
    g.pred_live = up(pred_live.data(), pred_live.size()); g.pred_live_n = (u32)pred_live.size();
    g.stat_out = up(stat_out.data(), stat_out.size()); g.stat_out_n = (u32)stat_out.size() / 2; g.d_out = up(d_out.data(), d_out.size()); g.d_out_n = (u32)d_out.size() / 2;
    {   // groups that can move the damage at all, for pool_prune.py; defense, health and the like only reach words the formula never reads
        std::vector<char> rel_stat(in_stat.size(), 0), rel_d(in_d.size(), 0);
        for (u32 i = 0; i < g.stat_out_n; i++) rel_stat[stat_out[2 * i]] = 1;
        for (u32 i = 0; i < g.d_out_n; i++) rel_d[d_out[2 * i]] = 1;
        for (u32 w = 0x24 / 4; w < 0x1A4 / 4; w++) rel_stat[w] = 1;
        rel_stat[0x7B8 / 4] = rel_stat[0x7A0 / 4] = 1;
        FILE* o = fopen((dir + "/live_groups.txt").c_str(), "w");
        for (u32 group = 0; group < klass.size(); group++) {
            const Klass& k = klass[group];
            bool relevant = k.kind == 3 || k.kind == 4;
            for (u32 i = k.start; k.kind == 2 && i < k.start + k.n; i++) relevant = relevant || (recipes[i].kind >= 6 ? rel_stat[recipes[i].offset / 4] : rel_d[recipes[i].offset / 4]);
            if (relevant) fprintf(o, "%u ", group);
        }
        fclose(o);
    }
    std::vector<Recipe> retained;
    for (Klass& k : klass) {
        if (k.kind != 2) continue;
        u32 start = (u32)retained.size();
        for (u32 i = k.start; i < k.start + k.n; i++) {
            Recipe r = recipes[i];
            bool on_stat = r.kind >= 6;
            bool live = false;
            for (u32 b = r.offset; b < r.offset + (r.width ? r.width : 4); b += 4) live = live || (on_stat ? in_stat[b / 4] : in_d[b / 4]);
            live = live || (on_stat ? in_stat[(r.offset + (r.width ? r.width : 4) - 1) / 4] : in_d[(r.offset + (r.width ? r.width : 4) - 1) / 4]);
            if (live) retained.push_back(r);
        }
        k.start = start; k.n = (u32)retained.size() - start;
    }
    printf("live words: STAT %zu D %zu pred %zu, outputs %u + %u, recipes %zu of %zu\n", stat_live.size(), d_live.size(), pred_live.size(), g.stat_out_n, g.d_out_n, retained.size(), recipes.size());
    recipes = retained;
    g.klass = up(klass.data(), klass.size()); g.recipes = up(recipes.data(), recipes.size());
    static std::vector<u16> slim_keep = slim_of;
    g.ctx.slim_of = up(slim_keep.data(), slim_keep.size());
#if TOUCH
    u8* touch; CHECK(cudaMalloc(&touch, full_words)); CHECK(cudaMemset(touch, 0, full_words)); g.ctx.touch = touch;
#endif
#if PROFILE
    u32* prof; CHECK(cudaMalloc(&prof, (2 * PROF_N + 2 * SLIM_WORDS) * 4)); CHECK(cudaMemset(prof, 0, (2 * PROF_N + 2 * SLIM_WORDS) * 4)); g.ctx.prof = prof;
#endif
    g.ctx.arena = up(arena.data(), arena.size());
    g.ctx.domain = up(DOM_SPANS, sizeof(DOM_SPANS) / sizeof(Span)); g.ctx.domains = sizeof(DOM_SPANS) / sizeof(Span);
    g.ctx.regions = up(REGIONS, sizeof(REGIONS) / sizeof(Span)); g.ctx.region_count = sizeof(REGIONS) / sizeof(Span);
    FILE* f = fopen((dir + "/gpu_setup.txt").c_str(), "r");
    u32 op_n; unsigned gain, defense;
    fscanf(f, "%u %u %u", &gain, &defense, &op_n);
    g.gain = gain; g.defense = defense; g.op_n = op_n;
    std::vector<u16> og(op_n); std::vector<i64> ov(op_n);
    for (u32 i = 0; i < op_n; i++) { unsigned a; long long b; fscanf(f, "%u %lld", &a, &b); og[i] = (u16)a; ov[i] = b; }
    g.op_group = up(og.data(), op_n); g.op_value = up(ov.data(), op_n);
    Space sp{}; int n_slots; fscanf(f, "%d", &n_slots); sp.n = n_slots; sp.keep = keep;
    for (int i = 0; i < n_slots; i++) fscanf(f, "%d %u %u", &sp.slot[i], &sp.start[i], &sp.radix[i]);
    for (int i = 0; i < 7; i++) fscanf(f, "%d", &sp.attrs[i]);
    fclose(f);
    std::vector<Cand> cases; std::vector<u32> want;
    f = fopen((dir + "/gpu_cases.txt").c_str(), "r");
    for (;;) { Cand k; unsigned bits; if (fscanf(f, "%x", &bits) != 1) break; for (int i = 0; i < 7; i++) fscanf(f, "%d", &k.attrs[i]); for (int i = 0; i < CAND_SLOTS; i++) fscanf(f, "%d", &k.slot[i]); cases.push_back(k); want.push_back(bits); }
    fclose(f);
    CHECK(cudaDeviceSetLimit(cudaLimitStackSize, 96 * 1024));
    printf("S %zu bytes, cases %zu\n", sizeof(S), cases.size()); fflush(stdout);
    const Cand* dc = up(cases.data(), cases.size());
    const u64 block = 128;
    u64 n = cases.size();
    u32* out; u8* ok;
    CHECK(cudaMalloc(&out, 4 * n)); CHECK(cudaMalloc(&ok, n));
    kernel_cases<<<(unsigned)((n + block - 1) / block), block>>>(g, dc, n, out, ok);
    CHECK(cudaDeviceSynchronize());
    std::vector<u32> got(n); std::vector<u8> fine(n);
    CHECK(cudaMemcpy(got.data(), out, 4 * n, cudaMemcpyDeviceToHost)); CHECK(cudaMemcpy(fine.data(), ok, n, cudaMemcpyDeviceToHost));
    long same = 0, wrong = 0, refused = 0, uncovered = 0;
    for (u64 i = 0; i < n; i++) {
        if (fine[i] == 0) refused++; else if (fine[i] == 2) uncovered++;
        else if (got[i] == want[i]) same++; else if (++wrong < 8) printf("DIFF case %llu got %08x want %08x\n", (unsigned long long)i, got[i], want[i]);
    }
    printf("cases %llu same %ld wrong %ld refused %ld uncovered %ld\n", (unsigned long long)n, same, wrong, refused, uncovered); fflush(stdout);
    if (argc > 3 && argv[3][0] == 'c') return 0;
    if (argc > 3 && argv[3][0] == 'd') {   // every formula input of each case, to see what separates two outfits
        u32* dumped; CHECK(cudaMalloc(&dumped, 4 * n * (SLIM_WORDS + 1)));
        kernel_dump<<<(unsigned)((n + block - 1) / block), block>>>(g, dc, n, dumped);
        CHECK(cudaDeviceSynchronize());
        std::vector<u32> rows(n * (SLIM_WORDS + 1));
        CHECK(cudaMemcpy(rows.data(), dumped, rows.size() * 4, cudaMemcpyDeviceToHost));
        for (u32 w = 0; w <= SLIM_WORDS; w++) {
            bool differs = false;
            for (u64 i = 1; i < n; i++) differs = differs || rows[i * (SLIM_WORDS + 1) + w] != rows[w];
            if (!differs) continue;
            u32 full = w < SLIM_WORDS ? SLIM_LIST[w] : 0xffffffffu;
            printf("word %x:", full * 4);
            for (u64 i = 0; i < n; i++) printf(" %08x", rows[i * (SLIM_WORDS + 1) + w]);
            printf("\n");
        }
        return 0;
    }
    sp.pipeline_only = argc > 3;
    // One space, or the disjoint spaces pool_prune.py wrote for a required set: which slots hold set pieces is what separates them.
    std::vector<Space> spaces;
    f = fopen((dir + "/gpu_spaces.txt").c_str(), "r");
    if (f) {
        int count; fscanf(f, "%d", &count);
        for (int k = 0; k < count; k++) {
            Space one = sp; one.id = k;
            for (int i = 0; i < one.n; i++) fscanf(f, "%d %u %u", &one.slot[i], &one.start[i], &one.radix[i]);
            spaces.push_back(one);
        }
        fclose(f);
    } else spaces.push_back(sp);
    u64 total = 0;
    for (const Space& one : spaces) { u64 size = 1; for (int i = 0; i < one.n; i++) size *= one.radix[i]; total += size; }
    const u32 capacity = 1 << 22;
    u32* best; Hit* hits; u32* hit_n; u32* counts; u64* missed; const u32 miss_capacity = 1 << 22;
    CHECK(cudaMalloc(&missed, miss_capacity * sizeof(u64)));
    CHECK(cudaMalloc(&best, 4)); CHECK(cudaMalloc(&hits, capacity * sizeof(Hit))); CHECK(cudaMalloc(&hit_n, 4)); CHECK(cudaMalloc(&counts, 16));
    const u64 batch = 1 << 22;
    {   // NIOH3_LIMITS "max_load min_defense min_health": anything outside is not an answer, in the search or in polish.
        const char* text = getenv("NIOH3_LIMITS");
        if (text && *text) { g.limits.on = 1; sscanf(text, "%f %u %u", &g.limits.max_load, &g.limits.min_defense, &g.limits.min_health); }
    }
    bool weapon_moves = false;
    for (const Space& one : spaces) for (int i = 0; i < one.n; i++) weapon_moves = weapon_moves || one.slot[i] == 0;
    const char* points = getenv("NIOH3_ATTRS");
    const bool move_points = points && *points;
    std::vector<Scored> leaders;
    u32 top, hit_count, cnt[4];
    for (int pass = 0; pass < (move_points ? 4 : 1); pass++) {
    CHECK(cudaMemset(best, 0, 4)); CHECK(cudaMemset(hit_n, 0, 4)); CHECK(cudaMemset(counts, 0, 16));
    g.seed = nullptr;
    if (!weapon_moves) {
        Cand q0; for (int a = 0; a < 7; a++) q0.attrs[a] = spaces[0].attrs[a]; for (int a = 0; a < CAND_SLOTS; a++) q0.slot[a] = -1;
        S* seed; u32* seed_ok; u32 fine = 0;
        CHECK(cudaMalloc(&seed, sizeof(S))); CHECK(cudaMalloc(&seed_ok, 4));
        kernel_seed<<<1, 1>>>(g, q0, seed, seed_ok);
        CHECK(cudaDeviceSynchronize());
        CHECK(cudaMemcpy(&fine, seed_ok, 4, cudaMemcpyDeviceToHost));
        if (fine && !getenv("NIOH3_NO_SEED")) g.seed = seed;
    }
    auto t = std::chrono::steady_clock::now();
    for (const Space& one : spaces) {
        u64 size = 1;
        for (int i = 0; i < one.n; i++) size *= one.radix[i];
        for (u64 at = 0; at < size; at += batch) {
            u64 span = std::min(batch, size - at);
            kernel_search<<<(unsigned)((span + block - 1) / block), block>>>(g, one, at, size, best, hits, hit_n, capacity, counts, missed, miss_capacity);
            CHECK(cudaDeviceSynchronize());
        }
    }
    double seconds = std::chrono::duration<double>(std::chrono::steady_clock::now() - t).count();
    CHECK(cudaMemcpy(&top, best, 4, cudaMemcpyDeviceToHost)); CHECK(cudaMemcpy(&hit_count, hit_n, 4, cudaMemcpyDeviceToHost)); CHECK(cudaMemcpy(cnt, counts, 16, cudaMemcpyDeviceToHost));
    printf("searched %llu in %.2fs: %.0f per second; best %.3f, hits %u, refused %u, no path %u, outside limits %u\n", (unsigned long long)total, seconds, total / seconds, bits_f(top), hit_count, cnt[0], cnt[2], cnt[3]);
#if PROFILE
    {
        std::vector<u32> pr(2 * PROF_N + 2 * SLIM_WORDS);
        CHECK(cudaMemcpy(pr.data(), prof, pr.size() * 4, cudaMemcpyDeviceToHost));
        FILE* o = fopen((dir + "/prof.txt").c_str(), "w");
        for (u32 i = 0; i < PROF_N; i++) fprintf(o, "path %u %u %u\n", i, pr[2 * i], pr[2 * i + 1]);
        for (u32 w = 0; w < SLIM_WORDS; w++) fprintf(o, "word %u %08x %08x %08x\n", (u32)SLIM_LIST[w], pr[2 * PROF_N + 2 * w], pr[2 * PROF_N + 2 * w + 1], masks[w]);
        fclose(o);
    }
#endif
#if TOUCH
    { std::vector<u8> seen(full_words); CHECK(cudaMemcpy(seen.data(), touch, full_words, cudaMemcpyDeviceToHost)); FILE* o = fopen((dir + "/touched.txt").c_str(), "w"); long c = 0; for (u32 i = 0; i < full_words; i++) if (seen[i]) { fprintf(o, "%u ", i); c++; } fclose(o); printf("touched by loads: %ld words\n", c); }
#endif
    u32 kept = std::min(hit_count, capacity);
    std::vector<Hit> all(kept);
    if (kept) CHECK(cudaMemcpy(all.data(), hits, kept * sizeof(Hit), cudaMemcpyDeviceToHost));
    std::sort(all.begin(), all.end(), [](const Hit& a, const Hit& b) { return bits_f(a.bits) > bits_f(b.bits); });
    g.seed = nullptr;
    for (u32 i = 0; i < kept && i < 400; i++) {
        Scored row{}; row.bits = all[i].bits;
        const Space& one = spaces[all[i].space];
        for (int a = 0; a < 7; a++) row.q.attrs[a] = one.attrs[a];
        for (int a = 0; a < CAND_SLOTS; a++) row.q.slot[a] = -1;
        u64 digits = all[i].index;
        for (int a = 0; a < one.n; a++) { row.q.slot[cand_index((u32)one.slot[a])] = (i32)(one.start[a] + (u32)(digits % one.radix[a])); digits /= one.radix[a]; }
        leaders.push_back(row);
    }
    polish(g, dir, leaders, move_points);
    // Better points can make other gear legal or better, so search again around them until the points settle.
    if (leaders.empty() || memcmp(leaders[0].q.attrs, spaces[0].attrs, sizeof(spaces[0].attrs)) == 0) break;
    if (move_points) {
        printf("points moved: %d %d %d %d %d %d %d, best %.3f\n", leaders[0].q.attrs[0], leaders[0].q.attrs[1], leaders[0].q.attrs[2], leaders[0].q.attrs[3], leaders[0].q.attrs[4], leaders[0].q.attrs[5], leaders[0].q.attrs[6], bits_f(leaders[0].bits));
        for (Space& one : spaces) memcpy(one.attrs, leaders[0].q.attrs, sizeof(one.attrs));
    }
    }
    f = fopen((dir + "/gpu_top.txt").c_str(), "w");
    for (size_t i = 0; i < leaders.size() && i < 40; i++) {
        fprintf(f, "%08x a:%d,%d,%d,%d,%d,%d,%d", leaders[i].bits, leaders[i].q.attrs[0], leaders[i].q.attrs[1], leaders[i].q.attrs[2], leaders[i].q.attrs[3], leaders[i].q.attrs[4], leaders[i].q.attrs[5], leaders[i].q.attrs[6]);
        for (int a = 0; a < CAND_SLOTS; a++) if (leaders[i].q.slot[a] >= 0) fprintf(f, " %d:%d", a == 9 ? 0 : a + 4, leaders[i].q.slot[a]);
        fprintf(f, "\n");
    }
    fclose(f);
    if (!leaders.empty()) {   // what the top outfits weigh and protect, so limits can be checked against the game
        std::vector<Cand> shown;
        for (size_t i = 0; i < leaders.size() && i < 40; i++) shown.push_back(leaders[i].q);
        u64 m = shown.size();
        const Cand* dc2 = up(shown.data(), m);
        float* load; u32* defense; u32* health;
        CHECK(cudaMalloc(&load, 4 * m)); CHECK(cudaMalloc(&defense, 4 * m)); CHECK(cudaMalloc(&health, 4 * m));
        kernel_facts<<<(unsigned)((m + block - 1) / block), block>>>(g, dc2, m, load, defense, health);
        CHECK(cudaDeviceSynchronize());
        std::vector<float> l(m); std::vector<u32> de(m), he(m);
        CHECK(cudaMemcpy(l.data(), load, 4 * m, cudaMemcpyDeviceToHost)); CHECK(cudaMemcpy(de.data(), defense, 4 * m, cudaMemcpyDeviceToHost)); CHECK(cudaMemcpy(he.data(), health, 4 * m, cudaMemcpyDeviceToHost));
        f = fopen((dir + "/gpu_facts.txt").c_str(), "w");
        for (u64 i = 0; i < m; i++) fprintf(f, "%.3f load %.4f defense %u health %u\n", bits_f(leaders[i].bits), l[i], de[i], he[i]);
        fclose(f);
    }
    u32 miss_n = std::min(cnt[2], miss_capacity);
    std::vector<u64> misses(miss_n);
    if (miss_n) CHECK(cudaMemcpy(misses.data(), missed, miss_n * sizeof(u64), cudaMemcpyDeviceToHost));
    std::sort(misses.begin(), misses.end());
    f = fopen((dir + "/gpu_nopath.txt").c_str(), "w");
    u32 step = std::max(1u, miss_n / 200);
    for (u32 i = 0; i < miss_n; i += step) {
        fprintf(f, "xxxxxxxx");
        const Space& one = spaces[misses[i] >> 48];
        u64 digits = misses[i] & 0xffffffffffffull;
        for (int a = 0; a < one.n; a++) { fprintf(f, " %d:%u", one.slot[a], one.start[a] + (u32)(digits % one.radix[a])); digits /= one.radix[a]; }
        fprintf(f, "\n");
    }
    fclose(f);
    return 0;
}
