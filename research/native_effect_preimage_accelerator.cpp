#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <algorithm>
#include <cstdint>
#include <cstring>
#include <vector>

#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>

#include "native_effect_filter_shader.h"
#include "native_preimage_shader.h"

using Microsoft::WRL::ComPtr;

namespace {

constexpr std::uint64_t ERROR_RESULT = 0xFFFFFFFFFFFFFFFFull;
constexpr std::uint32_t THREADS_PER_GROUP = 256u;
constexpr std::uint32_t MAX_GROUPS_PER_DISPATCH = 65535u;

#pragma pack(push, 1)
struct PathConstraintInput {
    std::uint32_t draw_index;
    std::uint32_t start_u16;
    std::uint32_t end_u16;
    std::uint32_t reserved;
};

struct EffectPathInput {
    std::int32_t promoted_slot;
    std::uint32_t constraint_count;
    std::uint32_t reserved0;
    std::uint32_t reserved1;
    PathConstraintInput constraints[6];
};

struct EffectCandidateInput {
    std::uint32_t effect_id;
    std::uint32_t group_key;
    std::uint32_t category_key;
    std::uint32_t conflict_mask_0;
    std::uint32_t conflict_mask_1;
    std::uint32_t normal_weight;
    std::uint32_t promoted_weight;
    std::uint32_t final_weight_common;
    std::uint32_t final_weight_special;
    std::uint32_t completion_candidate;
    std::uint32_t value_one_roll_mask;
};

struct SpecialGroupInput {
    std::uint32_t group_key;
    std::uint32_t conflict_mask_0;
    std::uint32_t conflict_mask_1;
    std::uint32_t effect_id;
};
#pragma pack(pop)

static_assert(sizeof(EffectPathInput) == 112u);
static_assert(sizeof(EffectCandidateInput) == 44u);
static_assert(sizeof(SpecialGroupInput) == 16u);

struct ShaderConstants {
    std::uint32_t dispatch_start_low;
    std::uint32_t dispatch_count;
    std::uint32_t pivot_value_count;
    std::uint32_t pivot_draw_index;
    std::uint32_t pivot_affine_addend;
    std::uint32_t pivot_inverse_multiplier;
    std::uint32_t promotion_draw_index;
    std::uint32_t promotion_threshold;
    std::uint32_t shuffle_draw_start;
    std::uint32_t rarity;
    std::uint32_t slot_limit;
    std::uint32_t max_draw_index;
    std::uint32_t path_count;
    std::uint32_t output_capacity;
    std::uint32_t reserved0;
    std::uint32_t reserved1;
};

struct EffectFilterConstants {
    std::uint32_t seed_count;
    std::uint32_t candidate_count;
    std::uint32_t criterion_group_count;
    std::uint32_t rarity;
    std::uint32_t ordinary_slot_count;
    std::uint32_t slot_limit;
    std::uint32_t promotion_threshold;
    std::uint32_t consumes_special_draw;
    std::uint32_t minimum_roll_percent;
    std::uint32_t maximum_roll_percent;
    std::uint32_t apply_r4_finalizer;
    std::uint32_t auxiliary_mode_threshold;
    std::uint32_t reserved[4];
};

constexpr char SHADER_SOURCE[] = R"HLSL(
struct EffectPath {
    int promoted_slot;
    uint constraint_count;
    uint reserved0;
    uint reserved1;
    uint4 constraints[6];
};

cbuffer Configuration : register(b0) {
    uint dispatch_start_low;
    uint dispatch_count;
    uint pivot_value_count;
    uint pivot_draw_index;
    uint pivot_affine_addend;
    uint pivot_inverse_multiplier;
    uint promotion_draw_index;
    uint promotion_threshold;
    uint shuffle_draw_start;
    uint rarity;
    uint slot_limit;
    uint max_draw_index;
    uint path_count;
    uint output_capacity;
    uint reserved0;
    uint reserved1;
};

StructuredBuffer<uint> PivotValues : register(t0);
StructuredBuffer<EffectPath> Paths : register(t1);
RWStructuredBuffer<uint2> Results : register(u0);
RWByteAddressBuffer Counter : register(u1);

uint random_int(uint value_u16, uint count) {
    float unit = float(value_u16) * (1.0f / 65536.0f);
    return min((uint)(unit * float(count)), count - 1u);
}

[numthreads(256, 1, 1)]
void main(uint3 dispatch_id : SV_DispatchThreadID) {
    uint local_index = dispatch_id.x;
    if (local_index >= dispatch_count) return;
    uint trial = dispatch_start_low + local_index;
    uint pivot_index = trial >> 16;
    if (pivot_index >= pivot_value_count) return;
    uint pivot_state = (PivotValues[pivot_index] << 16) | (trial & 0xFFFFu);
    uint seed = pivot_inverse_multiplier * (pivot_state - pivot_affine_addend);
    if ((seed & 0xF0000000u) != 0u || (seed & 0xFFFFu) == 0u) return;

    uint outputs[32];
    [unroll]
    for (uint clear_index = 0; clear_index < 32; ++clear_index) {
        outputs[clear_index] = 0u;
    }
    uint state = seed;
    [loop]
    for (uint draw = 1u; draw <= max_draw_index; ++draw) {
        state = state * 0x00010DCDu + 1u;
        outputs[draw] = state >> 16;
    }

    bool promoted = random_int(outputs[promotion_draw_index], 10000u) < promotion_threshold;
    int promoted_slot = -1;
    if (promoted) {
        int order[7] = {0, 1, 2, 3, 4, 5, 6};
        [unroll]
        for (uint position = 0u; position < 7u; ++position) {
            uint swap_index = random_int(outputs[shuffle_draw_start + position], 7u);
            int temporary = order[position];
            order[position] = order[swap_index];
            order[swap_index] = temporary;
        }
        [unroll]
        for (uint position = 0u; position < 7u; ++position) {
            int candidate = order[position];
            if (candidate >= int(slot_limit)) continue;
            if ((rarity == 4u || rarity == 5u) && candidate == 0) continue;
            promoted_slot = candidate;
            break;
        }
    }

    bool matched = false;
    [loop]
    for (uint path_index = 0u; path_index < path_count && !matched; ++path_index) {
        EffectPath path = Paths[path_index];
        if (path.promoted_slot != promoted_slot) continue;
        bool path_matches = true;
        [unroll]
        for (uint constraint_index = 0u; constraint_index < 6u; ++constraint_index) {
            if (constraint_index >= path.constraint_count) break;
            uint4 constraint = path.constraints[constraint_index];
            uint value = outputs[constraint.x];
            if (value < constraint.y || value > constraint.z) {
                path_matches = false;
                break;
            }
        }
        matched = path_matches;
    }
    if (!matched) return;

    uint output_index;
    Counter.InterlockedAdd(0u, 1u, output_index);
    if (output_index < output_capacity) {
        Results[output_index] = uint2(seed, trial);
    }
}
)HLSL";

constexpr char EFFECT_FILTER_SHADER_SOURCE[] = R"HLSL(
struct EffectCandidate {
    uint effect_id;
    uint group_key;
    uint category_key;
    uint conflict_mask_0;
    uint conflict_mask_1;
    uint normal_weight;
    uint promoted_weight;
    uint final_weight_common;
    uint final_weight_special;
    uint completion_candidate;
    uint value_one_roll_mask;
};

struct SpecialGroup {
    uint group_key;
    uint conflict_mask_0;
    uint conflict_mask_1;
    uint effect_id;
};

cbuffer EffectFilterConfiguration : register(b0) {
    uint seed_count;
    uint candidate_count;
    uint criterion_group_count;
    uint rarity;
    uint ordinary_slot_count;
    uint slot_limit;
    uint promotion_threshold;
    uint consumes_special_draw;
    uint minimum_roll_percent;
    uint maximum_roll_percent;
    uint apply_r4_finalizer;
    uint auxiliary_mode_threshold;
    uint reserved4;
    uint reserved5;
    uint reserved6;
    uint reserved7;
};

StructuredBuffer<uint> Seeds : register(t0);
StructuredBuffer<EffectCandidate> Candidates : register(t1);
StructuredBuffer<SpecialGroup> SpecialGroups : register(t2);
StructuredBuffer<uint> CategoryCapacities : register(t3);
StructuredBuffer<uint> CriterionKeys : register(t4);
StructuredBuffer<uint> CriterionOffsets : register(t5);
StructuredBuffer<uint> CriterionKinds : register(t6);
RWStructuredBuffer<uint> OutputMasks : register(u0);

uint lcg_step(uint state) {
    return state * 0x00010DCDu + 1u;
}

uint random_int(uint value_u16, uint count) {
    float unit = float(value_u16) * (1.0f / 65536.0f);
    return min((uint)(unit * float(count)), count - 1u);
}

bool groups_conflict(
    uint left_key,
    uint left_mask_0,
    uint left_mask_1,
    uint right_key,
    uint right_mask_0,
    uint right_mask_1) {
    return left_key == right_key ||
        (left_mask_0 & right_mask_0) != 0u ||
        (left_mask_1 & right_mask_1) != 0u;
}

bool candidate_allowed(
    EffectCandidate candidate,
    bool promoted,
    uint capacities[32],
    SpecialGroup special_group,
    uint accepted_count,
    uint accepted_group_keys[5],
    uint accepted_masks_0[5],
    uint accepted_masks_1[5]) {
    uint weight = promoted ? candidate.promoted_weight : candidate.normal_weight;
    if (weight == 0u || candidate.category_key >= 32u ||
        capacities[candidate.category_key] == 0u) {
        return false;
    }
    if (groups_conflict(
            candidate.group_key,
            candidate.conflict_mask_0,
            candidate.conflict_mask_1,
            special_group.group_key,
            special_group.conflict_mask_0,
            special_group.conflict_mask_1)) {
        return false;
    }
    [unroll]
    for (uint index = 0u; index < 5u; ++index) {
        if (index >= accepted_count) break;
        if (groups_conflict(
                candidate.group_key,
                candidate.conflict_mask_0,
                candidate.conflict_mask_1,
                accepted_group_keys[index],
                accepted_masks_0[index],
                accepted_masks_1[index])) {
            return false;
        }
    }
    return true;
}

uint mark_effect_constraints(uint effect_id, uint position, uint matched_mask) {
    [loop]
    for (uint group = 0u; group < criterion_group_count; ++group) {
        if ((matched_mask & (1u << group)) != 0u) continue;
        uint kind = CriterionKinds[group];
        bool eligible = kind == 2u ||
            (kind == 0u && position == 0u) ||
            (kind == 1u && position != 0u);
        if (!eligible) continue;
        [loop]
        for (uint index = CriterionOffsets[group];
             index < CriterionOffsets[group + 1u]; ++index) {
            if (CriterionKeys[index] == effect_id) {
                matched_mask |= 1u << group;
                break;
            }
        }
    }
    return matched_mask;
}
)HLSL"
R"HLSL(

uint roll_percentile(uint first_u16, uint second_u16) {
    if (minimum_roll_percent >= maximum_roll_percent) {
        return minimum_roll_percent;
    }
    uint first = random_int(first_u16, 46u);
    uint second = random_int(second_u16, 46u);
    uint lottery = first + second + (first == second ? 10u : 0u);
    float product = float(lottery) * float(maximum_roll_percent - minimum_roll_percent);
    float scaled = product / 100.0f;
    return minimum_roll_percent + (uint)scaled;
}

bool uses_special_finalizer_weight(uint display_seed) {
    uint scoped_seed =
        ((display_seed & 0x01E3C78Fu) << 3u) |
        ((display_seed >> 4u) & 0x00E1C387u);
    uint state = lcg_step(scoped_seed);
    uint first_roll = random_int(state >> 16u, 10000u);
    uint branch_class = 2u;
    if (first_roll >= auxiliary_mode_threshold) {
        state = lcg_step(state);
        branch_class = random_int(state >> 16u, 2u) == 0u ? 1u : 0u;
    }
    state = lcg_step(state);
    uint matching_count = branch_class == 2u ? 3u : 2u;
    uint selected = random_int(state >> 16u, matching_count);
    return branch_class == 0u || (branch_class == 1u && selected == 1u);
}

bool finalizer_candidate_allowed(
    EffectCandidate candidate,
    uint target_index,
    uint source_effect_ids[5],
    uint source_group_keys[5],
    uint source_masks_0[5],
    uint source_masks_1[5],
    uint source_categories[5],
    int prior_candidate_indices[4],
    uint prior_count) {
    if (candidate.effect_id == source_effect_ids[target_index] ||
        candidate.category_key >= 32u) {
        return false;
    }
    uint used_capacity = 0u;
    [unroll]
    for (uint index = 0u; index < 4u; ++index) {
        if (index != target_index &&
            source_categories[index] == candidate.category_key) {
            used_capacity += 1u;
        }
    }
    if (used_capacity >= CategoryCapacities[candidate.category_key]) {
        return false;
    }
    [unroll]
    for (uint index = 0u; index < 5u; ++index) {
        if (index == target_index) continue;
        if (groups_conflict(
                candidate.group_key,
                candidate.conflict_mask_0,
                candidate.conflict_mask_1,
                source_group_keys[index],
                source_masks_0[index],
                source_masks_1[index])) {
            return false;
        }
    }
    [unroll]
    for (uint index = 0u; index < 4u; ++index) {
        if (index >= prior_count) break;
        EffectCandidate prior = Candidates[prior_candidate_indices[index]];
        if (groups_conflict(
                candidate.group_key,
                candidate.conflict_mask_0,
                candidate.conflict_mask_1,
                prior.group_key,
                prior.conflict_mask_0,
                prior.conflict_mask_1)) {
            return false;
        }
    }
    return true;
}

int select_finalizer_candidate(
    uint display_seed,
    uint target_index,
    uint source_effect_ids[5],
    uint source_rolls[5],
    uint source_group_keys[5],
    uint source_masks_0[5],
    uint source_masks_1[5],
    uint source_categories[5],
    int prior_candidate_indices[4],
    uint prior_count,
    bool use_special_weight) {
    uint state = display_seed + 7u * (target_index << 16u);
    [unroll]
    for (uint index = 0u; index < 5u; ++index) {
        state += source_effect_ids[index] * min(source_rolls[index], 100u);
    }
    [unroll]
    for (uint index = 0u; index < 4u; ++index) {
        if (index >= target_index) break;
        state = lcg_step(state);
    }

    // Clearing one occupied source slot always leaves one mode-0x12 category
    // assignment candidate. Its selected category is subsequently overwritten,
    // but the native assignment consumes this one RNG draw.
    state = lcg_step(state);

    uint total_weight = 0u;
    [loop]
    for (uint row_index = 0u; row_index < candidate_count; ++row_index) {
        EffectCandidate candidate = Candidates[row_index];
        uint weight = use_special_weight
            ? candidate.final_weight_special
            : candidate.final_weight_common;
        if (weight == 0u || !finalizer_candidate_allowed(
                candidate,
                target_index,
                source_effect_ids,
                source_group_keys,
                source_masks_0,
                source_masks_1,
                source_categories,
                prior_candidate_indices,
                prior_count)) {
            continue;
        }
        total_weight += weight;
    }
    if (total_weight == 0u || total_weight == 0xFFFFFFFFu) {
        return -1;
    }
    state = lcg_step(state);
    uint ticket = random_int(state >> 16u, total_weight + 1u);
    ticket = min(ticket, total_weight);
    [loop]
    for (uint row_index = 0u; row_index < candidate_count; ++row_index) {
        EffectCandidate candidate = Candidates[row_index];
        uint weight = use_special_weight
            ? candidate.final_weight_special
            : candidate.final_weight_common;
        if (weight == 0u || !finalizer_candidate_allowed(
                candidate,
                target_index,
                source_effect_ids,
                source_group_keys,
                source_masks_0,
                source_masks_1,
                source_categories,
                prior_candidate_indices,
                prior_count)) {
            continue;
        }
        if (ticket <= weight) {
            return int(row_index);
        }
        ticket -= weight;
    }
    return -1;
}

int build_completion_candidate(
    uint display_seed,
    uint target_index,
    uint source_effect_ids[5],
    uint source_rolls[5],
    int source_candidate_indices[5],
    uint source_group_keys[5],
    uint source_masks_0[5],
    uint source_masks_1[5],
    uint source_categories[5],
    bool use_special_weight) {
    int prior_candidate_indices[4] = {-1, -1, -1, -1};
    uint prior_count = 0u;
    [unroll]
    for (uint prior_index = 1u; prior_index < 4u; ++prior_index) {
        if (prior_index >= target_index) break;
        EffectCandidate source = Candidates[source_candidate_indices[prior_index]];
        uint roll_offset = source_rolls[prior_index] - minimum_roll_percent;
        bool source_value_is_one = roll_offset < 32u &&
            (source.value_one_roll_mask & (1u << roll_offset)) != 0u;
        if (source_value_is_one) continue;
        int selected = select_finalizer_candidate(
            display_seed,
            prior_index,
            source_effect_ids,
            source_rolls,
            source_group_keys,
            source_masks_0,
            source_masks_1,
            source_categories,
            prior_candidate_indices,
            prior_count,
            use_special_weight);
        if (selected >= 0) {
            prior_candidate_indices[prior_count] = selected;
            prior_count += 1u;
        }
    }
    return select_finalizer_candidate(
        display_seed,
        target_index,
        source_effect_ids,
        source_rolls,
        source_group_keys,
        source_masks_0,
        source_masks_1,
        source_categories,
        prior_candidate_indices,
        prior_count,
        use_special_weight);
}

uint mark_r4_final_effect_constraints(
    uint display_seed,
    uint source_effect_ids[5],
    uint source_rolls[5],
    int source_candidate_indices[5],
    uint source_group_keys[5],
    uint source_masks_0[5],
    uint source_masks_1[5],
    uint source_categories[5],
    uint source_effect_flags[5]) {
    uint final_effect_ids[5];
    [unroll]
    for (uint index = 0u; index < 5u; ++index) {
        final_effect_ids[index] = source_effect_ids[index];
    }
    bool use_special_weight = uses_special_finalizer_weight(display_seed);
    [unroll]
    for (uint target_index = 1u; target_index < 5u; ++target_index) {
        if ((source_effect_flags[target_index] & 0x04u) != 0u) continue;
        int selected = build_completion_candidate(
            display_seed,
            target_index,
            source_effect_ids,
            source_rolls,
            source_candidate_indices,
            source_group_keys,
            source_masks_0,
            source_masks_1,
            source_categories,
            use_special_weight);
        if (selected >= 0 && Candidates[selected].completion_candidate != 0u) {
            final_effect_ids[target_index] = Candidates[selected].effect_id;
            break;
        }
    }
    uint matched_mask = 0u;
    [unroll]
    for (uint position = 0u; position < 5u; ++position) {
        matched_mask = mark_effect_constraints(
            final_effect_ids[position],
            position,
            matched_mask);
    }
    return matched_mask;
}
)HLSL"
R"HLSL(

[numthreads(128, 1, 1)]
void main(uint3 dispatch_id : SV_DispatchThreadID) {
    uint seed_index = dispatch_id.x;
    if (seed_index >= seed_count) return;
    uint state = Seeds[seed_index];
    SpecialGroup special_group = SpecialGroups[0];
    if (consumes_special_draw != 0u) {
        state = lcg_step(state);
        special_group = SpecialGroups[state >> 16u];
    }
    state = lcg_step(state);
    bool promoted = random_int(state >> 16u, 10000u) < promotion_threshold;
    int promoted_slot = -1;
    if (promoted) {
        int order[7] = {0, 1, 2, 3, 4, 5, 6};
        [unroll]
        for (uint position = 0u; position < 7u; ++position) {
            state = lcg_step(state);
            uint swap_index = random_int(state >> 16u, 7u);
            int temporary = order[position];
            order[position] = order[swap_index];
            order[swap_index] = temporary;
        }
        [unroll]
        for (uint position = 0u; position < 7u; ++position) {
            int candidate = order[position];
            if (candidate >= int(slot_limit)) continue;
            if (consumes_special_draw != 0u && candidate == 0) continue;
            promoted_slot = candidate;
            break;
        }
    }

    uint capacities[32];
    [unroll]
    for (uint index = 0u; index < 32u; ++index) {
        capacities[index] = CategoryCapacities[index];
    }
    uint accepted_group_keys[5] = {0u, 0u, 0u, 0u, 0u};
    uint accepted_masks_0[5] = {0u, 0u, 0u, 0u, 0u};
    uint accepted_masks_1[5] = {0u, 0u, 0u, 0u, 0u};
    uint source_effect_ids[5] = {0u, 0u, 0u, 0u, 0u};
    uint source_rolls[5] = {0u, 0u, 0u, 0u, 0u};
    int source_candidate_indices[5] = {-1, -1, -1, -1, -1};
    uint source_categories[5] = {0u, 0u, 0u, 0u, 0u};
    uint source_effect_flags[5] = {0u, 0u, 0u, 0u, 0u};
    uint matched_mask = 0u;

    [loop]
    for (uint position = 0u; position < ordinary_slot_count; ++position) {
        uint source_slot = consumes_special_draw != 0u ? position + 1u : position;
        bool slot_promoted = promoted_slot == int(source_slot);
        uint total_weight = 0u;
        [loop]
        for (uint row_index = 0u; row_index < candidate_count; ++row_index) {
            EffectCandidate candidate = Candidates[row_index];
            if (!candidate_allowed(
                    candidate,
                    slot_promoted,
                    capacities,
                    special_group,
                    position,
                    accepted_group_keys,
                    accepted_masks_0,
                    accepted_masks_1)) {
                continue;
            }
            total_weight += slot_promoted
                ? candidate.promoted_weight
                : candidate.normal_weight;
        }
        if (total_weight == 0u || total_weight == 0xFFFFFFFFu) {
            OutputMasks[seed_index] = 0u;
            return;
        }
        state = lcg_step(state);
        uint ticket = random_int(state >> 16u, total_weight + 1u);
        ticket = min(ticket, total_weight);
        int selected_index = -1;
        [loop]
        for (uint row_index = 0u; row_index < candidate_count; ++row_index) {
            EffectCandidate candidate = Candidates[row_index];
            if (!candidate_allowed(
                    candidate,
                    slot_promoted,
                    capacities,
                    special_group,
                    position,
                    accepted_group_keys,
                    accepted_masks_0,
                    accepted_masks_1)) {
                continue;
            }
            uint weight = slot_promoted
                ? candidate.promoted_weight
                : candidate.normal_weight;
            if (ticket <= weight) {
                selected_index = int(row_index);
                break;
            }
            ticket -= weight;
        }
        if (selected_index < 0) {
            OutputMasks[seed_index] = 0u;
            return;
        }
        EffectCandidate selected = Candidates[selected_index];
        accepted_group_keys[position] = selected.group_key;
        accepted_masks_0[position] = selected.conflict_mask_0;
        accepted_masks_1[position] = selected.conflict_mask_1;
        source_effect_ids[position] = selected.effect_id;
        source_candidate_indices[position] = selected_index;
        source_categories[position] = selected.category_key;
        source_effect_flags[position] = slot_promoted ? 0x04u : 0u;
        capacities[selected.category_key] -= 1u;
        state = lcg_step(state);
        uint first_roll_u16 = state >> 16u;
        state = lcg_step(state);
        uint second_roll_u16 = state >> 16u;
        source_rolls[position] = roll_percentile(
            first_roll_u16,
            second_roll_u16);
        if (apply_r4_finalizer == 0u) {
            matched_mask = mark_effect_constraints(
                selected.effect_id,
                position,
                matched_mask);
        }
    }
    if (apply_r4_finalizer != 0u) {
        source_effect_ids[4] = special_group.effect_id;
        source_rolls[4] = 0u;
        source_candidate_indices[4] = -1;
        accepted_group_keys[4] = special_group.group_key;
        accepted_masks_0[4] = special_group.conflict_mask_0;
        accepted_masks_1[4] = special_group.conflict_mask_1;
        source_categories[4] = 0u;
        source_effect_flags[4] = 0x02u;
        matched_mask = mark_r4_final_effect_constraints(
            Seeds[seed_index],
            source_effect_ids,
            source_rolls,
            source_candidate_indices,
            accepted_group_keys,
            accepted_masks_0,
            accepted_masks_1,
            source_categories,
            source_effect_flags);
    }
    OutputMasks[seed_index] = matched_mask;
}
)HLSL";

struct DeviceBundle {
    ComPtr<IDXGIAdapter1> adapter;
    ComPtr<ID3D11Device> device;
    ComPtr<ID3D11DeviceContext> context;
    std::uint32_t vendor_id = 0u;
};

bool create_device(std::uint32_t preferred_vendor_id, DeviceBundle* output) {
    ComPtr<IDXGIFactory1> factory;
    if (FAILED(CreateDXGIFactory1(IID_PPV_ARGS(&factory)))) return false;

    std::vector<ComPtr<IDXGIAdapter1>> candidates;
    for (UINT index = 0;; ++index) {
        ComPtr<IDXGIAdapter1> adapter;
        if (factory->EnumAdapters1(index, &adapter) == DXGI_ERROR_NOT_FOUND) break;
        DXGI_ADAPTER_DESC1 description{};
        if (FAILED(adapter->GetDesc1(&description))) continue;
        if (description.Flags & DXGI_ADAPTER_FLAG_SOFTWARE) continue;
        if (preferred_vendor_id != 0u && description.VendorId != preferred_vendor_id) continue;
        candidates.push_back(adapter);
    }
    if (candidates.empty()) return false;
    std::sort(
        candidates.begin(),
        candidates.end(),
        [](const ComPtr<IDXGIAdapter1>& left, const ComPtr<IDXGIAdapter1>& right) {
            DXGI_ADAPTER_DESC1 left_description{};
            DXGI_ADAPTER_DESC1 right_description{};
            left->GetDesc1(&left_description);
            right->GetDesc1(&right_description);
            return left_description.DedicatedVideoMemory > right_description.DedicatedVideoMemory;
        });

    const D3D_FEATURE_LEVEL requested_levels[] = {
        D3D_FEATURE_LEVEL_11_1,
        D3D_FEATURE_LEVEL_11_0,
    };
    for (const auto& adapter : candidates) {
        ComPtr<ID3D11Device> device;
        ComPtr<ID3D11DeviceContext> context;
        D3D_FEATURE_LEVEL actual_level{};
        HRESULT result = D3D11CreateDevice(
            adapter.Get(),
            D3D_DRIVER_TYPE_UNKNOWN,
            nullptr,
            0u,
            requested_levels,
            ARRAYSIZE(requested_levels),
            D3D11_SDK_VERSION,
            &device,
            &actual_level,
            &context);
        if (FAILED(result)) continue;
        DXGI_ADAPTER_DESC1 description{};
        adapter->GetDesc1(&description);
        output->adapter = adapter;
        output->device = device;
        output->context = context;
        output->vendor_id = description.VendorId;
        return true;
    }
    return false;
}

template <typename T>
bool create_structured_buffer(
    ID3D11Device* device,
    const T* values,
    std::uint32_t count,
    ComPtr<ID3D11Buffer>* buffer,
    ComPtr<ID3D11ShaderResourceView>* view) {
    if (count == 0u) return false;
    D3D11_BUFFER_DESC description{};
    description.ByteWidth = sizeof(T) * count;
    description.Usage = D3D11_USAGE_IMMUTABLE;
    description.BindFlags = D3D11_BIND_SHADER_RESOURCE;
    description.MiscFlags = D3D11_RESOURCE_MISC_BUFFER_STRUCTURED;
    description.StructureByteStride = sizeof(T);
    D3D11_SUBRESOURCE_DATA initial{};
    initial.pSysMem = values;
    if (FAILED(device->CreateBuffer(&description, &initial, buffer->GetAddressOf()))) return false;
    D3D11_SHADER_RESOURCE_VIEW_DESC view_description{};
    view_description.Format = DXGI_FORMAT_UNKNOWN;
    view_description.ViewDimension = D3D11_SRV_DIMENSION_BUFFER;
    view_description.Buffer.NumElements = count;
    return SUCCEEDED(
        device->CreateShaderResourceView(
            buffer->Get(),
            &view_description,
            view->GetAddressOf()));
}

bool create_output_buffer(
    ID3D11Device* device,
    std::uint32_t capacity,
    ComPtr<ID3D11Buffer>* buffer,
    ComPtr<ID3D11UnorderedAccessView>* view) {
    D3D11_BUFFER_DESC description{};
    description.ByteWidth = capacity * sizeof(std::uint32_t) * 2u;
    description.Usage = D3D11_USAGE_DEFAULT;
    description.BindFlags = D3D11_BIND_UNORDERED_ACCESS;
    description.MiscFlags = D3D11_RESOURCE_MISC_BUFFER_STRUCTURED;
    description.StructureByteStride = sizeof(std::uint32_t) * 2u;
    if (FAILED(device->CreateBuffer(&description, nullptr, buffer->GetAddressOf()))) return false;
    D3D11_UNORDERED_ACCESS_VIEW_DESC view_description{};
    view_description.Format = DXGI_FORMAT_UNKNOWN;
    view_description.ViewDimension = D3D11_UAV_DIMENSION_BUFFER;
    view_description.Buffer.NumElements = capacity;
    return SUCCEEDED(
        device->CreateUnorderedAccessView(
            buffer->Get(),
            &view_description,
            view->GetAddressOf()));
}

bool create_output_u32_buffer(
    ID3D11Device* device,
    std::uint32_t count,
    ComPtr<ID3D11Buffer>* buffer,
    ComPtr<ID3D11UnorderedAccessView>* view) {
    D3D11_BUFFER_DESC description{};
    description.ByteWidth = count * sizeof(std::uint32_t);
    description.Usage = D3D11_USAGE_DEFAULT;
    description.BindFlags = D3D11_BIND_UNORDERED_ACCESS;
    description.MiscFlags = D3D11_RESOURCE_MISC_BUFFER_STRUCTURED;
    description.StructureByteStride = sizeof(std::uint32_t);
    if (FAILED(device->CreateBuffer(
            &description,
            nullptr,
            buffer->GetAddressOf()))) {
        return false;
    }
    D3D11_UNORDERED_ACCESS_VIEW_DESC view_description{};
    view_description.Format = DXGI_FORMAT_UNKNOWN;
    view_description.ViewDimension = D3D11_UAV_DIMENSION_BUFFER;
    view_description.Buffer.NumElements = count;
    return SUCCEEDED(device->CreateUnorderedAccessView(
        buffer->Get(),
        &view_description,
        view->GetAddressOf()));
}

bool create_counter_buffer(
    ID3D11Device* device,
    ComPtr<ID3D11Buffer>* buffer,
    ComPtr<ID3D11UnorderedAccessView>* view) {
    const std::uint32_t zero = 0u;
    D3D11_BUFFER_DESC description{};
    description.ByteWidth = sizeof(zero);
    description.Usage = D3D11_USAGE_DEFAULT;
    description.BindFlags = D3D11_BIND_UNORDERED_ACCESS;
    description.MiscFlags = D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS;
    D3D11_SUBRESOURCE_DATA initial{};
    initial.pSysMem = &zero;
    if (FAILED(device->CreateBuffer(&description, &initial, buffer->GetAddressOf()))) return false;
    D3D11_UNORDERED_ACCESS_VIEW_DESC view_description{};
    view_description.Format = DXGI_FORMAT_R32_TYPELESS;
    view_description.ViewDimension = D3D11_UAV_DIMENSION_BUFFER;
    view_description.Buffer.Flags = D3D11_BUFFER_UAV_FLAG_RAW;
    view_description.Buffer.NumElements = 1u;
    return SUCCEEDED(
        device->CreateUnorderedAccessView(
            buffer->Get(),
            &view_description,
            view->GetAddressOf()));
}

template <typename T>
bool read_buffer(
    ID3D11Device* device,
    ID3D11DeviceContext* context,
    ID3D11Buffer* source,
    T* destination,
    std::uint32_t byte_count) {
    D3D11_BUFFER_DESC description{};
    source->GetDesc(&description);
    description.Usage = D3D11_USAGE_STAGING;
    description.BindFlags = 0u;
    description.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
    description.MiscFlags = 0u;
    description.StructureByteStride = 0u;
    ComPtr<ID3D11Buffer> staging;
    if (FAILED(device->CreateBuffer(&description, nullptr, &staging))) return false;
    context->CopyResource(staging.Get(), source);
    D3D11_MAPPED_SUBRESOURCE mapped{};
    if (FAILED(context->Map(staging.Get(), 0u, D3D11_MAP_READ, 0u, &mapped))) return false;
    std::memcpy(destination, mapped.pData, byte_count);
    context->Unmap(staging.Get(), 0u);
    return true;
}

}  // namespace

extern "C" __declspec(dllexport) int d3d11_effect_acceleration_available(
    std::uint32_t preferred_vendor_id) {
    DeviceBundle bundle;
    return create_device(preferred_vendor_id, &bundle) ? 1 : 0;
}

extern "C" __declspec(dllexport) int d3d11_effect_adapter_info(
    std::uint32_t preferred_vendor_id,
    std::uint32_t* output_vendor_id,
    std::uint32_t* output_device_id,
    std::uint64_t* output_dedicated_video_memory,
    std::uint64_t* output_shared_system_memory,
    wchar_t* output_description,
    std::uint32_t description_capacity) {
    if (
        output_vendor_id == nullptr || output_device_id == nullptr ||
        output_dedicated_video_memory == nullptr ||
        output_shared_system_memory == nullptr || output_description == nullptr ||
        description_capacity == 0u) {
        return 0;
    }
    DeviceBundle bundle;
    if (!create_device(preferred_vendor_id, &bundle)) return 0;
    DXGI_ADAPTER_DESC1 description{};
    if (FAILED(bundle.adapter->GetDesc1(&description))) return 0;
    *output_vendor_id = description.VendorId;
    *output_device_id = description.DeviceId;
    *output_dedicated_video_memory = description.DedicatedVideoMemory;
    *output_shared_system_memory = description.SharedSystemMemory;
    wcsncpy_s(
        output_description,
        description_capacity,
        description.Description,
        _TRUNCATE);
    return 1;
}

extern "C" __declspec(dllexport) std::uint64_t collect_effect_preimage_matches_d3d11(
    const std::uint16_t* pivot_values,
    std::uint32_t pivot_value_count,
    std::uint64_t start_trial,
    std::uint64_t stop_trial,
    std::uint32_t pivot_draw_index,
    std::uint32_t pivot_affine_addend,
    std::uint32_t pivot_inverse_multiplier,
    std::uint32_t promotion_draw_index,
    std::uint32_t promotion_threshold,
    std::uint32_t shuffle_draw_start,
    std::uint32_t rarity,
    std::uint32_t slot_limit,
    std::uint32_t max_draw_index,
    const EffectPathInput* paths,
    std::uint32_t path_count,
    std::uint32_t preferred_vendor_id,
    std::uint32_t* output_seeds,
    std::uint64_t* output_trials,
    std::uint32_t output_capacity,
    std::uint32_t* output_vendor_id) {
    if (
        pivot_values == nullptr || pivot_value_count == 0u ||
        start_trial > stop_trial || stop_trial > (std::uint64_t(pivot_value_count) << 16u) ||
        pivot_draw_index == 0u || promotion_draw_index == 0u ||
        shuffle_draw_start == 0u || max_draw_index >= 32u ||
        paths == nullptr || path_count == 0u ||
        output_seeds == nullptr || output_trials == nullptr ||
        output_capacity == 0u || output_vendor_id == nullptr) {
        return ERROR_RESULT;
    }
    if (start_trial == stop_trial) {
        *output_vendor_id = 0u;
        return 0u;
    }

    DeviceBundle bundle;
    if (!create_device(preferred_vendor_id, &bundle)) return ERROR_RESULT;
    *output_vendor_id = bundle.vendor_id;

    ComPtr<ID3D11ComputeShader> shader;
    if (FAILED(bundle.device->CreateComputeShader(
            g_preimage_shader_bytecode,
            sizeof(g_preimage_shader_bytecode),
            nullptr,
            &shader))) {
        return ERROR_RESULT;
    }

    std::vector<std::uint32_t> pivot_values_u32(pivot_value_count);
    std::transform(
        pivot_values,
        pivot_values + pivot_value_count,
        pivot_values_u32.begin(),
        [](std::uint16_t value) { return std::uint32_t(value); });
    ComPtr<ID3D11Buffer> pivot_buffer;
    ComPtr<ID3D11ShaderResourceView> pivot_view;
    ComPtr<ID3D11Buffer> path_buffer;
    ComPtr<ID3D11ShaderResourceView> path_view;
    if (
        !create_structured_buffer(
            bundle.device.Get(),
            pivot_values_u32.data(),
            pivot_value_count,
            &pivot_buffer,
            &pivot_view) ||
        !create_structured_buffer(
            bundle.device.Get(),
            paths,
            path_count,
            &path_buffer,
            &path_view)) {
        return ERROR_RESULT;
    }

    ComPtr<ID3D11Buffer> result_buffer;
    ComPtr<ID3D11UnorderedAccessView> result_view;
    ComPtr<ID3D11Buffer> counter_buffer;
    ComPtr<ID3D11UnorderedAccessView> counter_view;
    if (
        !create_output_buffer(bundle.device.Get(), output_capacity, &result_buffer, &result_view) ||
        !create_counter_buffer(bundle.device.Get(), &counter_buffer, &counter_view)) {
        return ERROR_RESULT;
    }

    D3D11_BUFFER_DESC constant_description{};
    constant_description.ByteWidth = sizeof(ShaderConstants);
    constant_description.Usage = D3D11_USAGE_DYNAMIC;
    constant_description.BindFlags = D3D11_BIND_CONSTANT_BUFFER;
    constant_description.CPUAccessFlags = D3D11_CPU_ACCESS_WRITE;
    ComPtr<ID3D11Buffer> constant_buffer;
    if (FAILED(bundle.device->CreateBuffer(
            &constant_description,
            nullptr,
            &constant_buffer))) {
        return ERROR_RESULT;
    }

    ID3D11ShaderResourceView* shader_views[] = {pivot_view.Get(), path_view.Get()};
    ID3D11UnorderedAccessView* unordered_views[] = {result_view.Get(), counter_view.Get()};
    bundle.context->CSSetShader(shader.Get(), nullptr, 0u);
    bundle.context->CSSetShaderResources(0u, ARRAYSIZE(shader_views), shader_views);
    bundle.context->CSSetUnorderedAccessViews(
        0u,
        ARRAYSIZE(unordered_views),
        unordered_views,
        nullptr);
    bundle.context->CSSetConstantBuffers(0u, 1u, constant_buffer.GetAddressOf());

    std::uint64_t dispatch_start = start_trial;
    while (dispatch_start < stop_trial) {
        const std::uint64_t maximum_threads =
            std::uint64_t(THREADS_PER_GROUP) * MAX_GROUPS_PER_DISPATCH;
        const std::uint32_t dispatch_count = static_cast<std::uint32_t>(
            std::min(stop_trial - dispatch_start, maximum_threads));
        ShaderConstants constants{};
        constants.dispatch_start_low = static_cast<std::uint32_t>(dispatch_start);
        constants.dispatch_count = dispatch_count;
        constants.pivot_value_count = pivot_value_count;
        constants.pivot_draw_index = pivot_draw_index;
        constants.pivot_affine_addend = pivot_affine_addend;
        constants.pivot_inverse_multiplier = pivot_inverse_multiplier;
        constants.promotion_draw_index = promotion_draw_index;
        constants.promotion_threshold = promotion_threshold;
        constants.shuffle_draw_start = shuffle_draw_start;
        constants.rarity = rarity;
        constants.slot_limit = slot_limit;
        constants.max_draw_index = max_draw_index;
        constants.path_count = path_count;
        constants.output_capacity = output_capacity;
        D3D11_MAPPED_SUBRESOURCE mapped{};
        if (FAILED(bundle.context->Map(
                constant_buffer.Get(),
                0u,
                D3D11_MAP_WRITE_DISCARD,
                0u,
                &mapped))) {
            return ERROR_RESULT;
        }
        std::memcpy(mapped.pData, &constants, sizeof(constants));
        bundle.context->Unmap(constant_buffer.Get(), 0u);
        const std::uint32_t groups =
            (dispatch_count + THREADS_PER_GROUP - 1u) / THREADS_PER_GROUP;
        bundle.context->Dispatch(groups, 1u, 1u);
        dispatch_start += dispatch_count;
    }
    bundle.context->Flush();

    std::uint32_t count = 0u;
    if (!read_buffer(
            bundle.device.Get(),
            bundle.context.Get(),
            counter_buffer.Get(),
            &count,
            sizeof(count))) {
        return ERROR_RESULT;
    }
    if (count > output_capacity) return ERROR_RESULT;
    if (count == 0u) return 0u;
    std::vector<std::uint32_t> pairs(count * 2u);
    if (!read_buffer(
            bundle.device.Get(),
            bundle.context.Get(),
            result_buffer.Get(),
            pairs.data(),
            count * sizeof(std::uint32_t) * 2u)) {
        return ERROR_RESULT;
    }
    for (std::uint32_t index = 0u; index < count; ++index) {
        output_seeds[index] = pairs[index * 2u];
        output_trials[index] = pairs[index * 2u + 1u];
    }
    return count;
}

extern "C" __declspec(dllexport) int match_effect_constraints_d3d11(
    const std::uint32_t* seeds,
    std::uint32_t seed_count,
    const EffectCandidateInput* candidates,
    std::uint32_t candidate_count,
    const SpecialGroupInput* special_groups,
    std::uint32_t special_group_count,
    const std::uint32_t* category_capacities,
    const std::uint32_t* criterion_keys,
    std::uint32_t criterion_key_count,
    const std::uint32_t* criterion_offsets,
    const std::uint32_t* criterion_kinds,
    std::uint32_t criterion_group_count,
    std::uint32_t rarity,
    std::uint32_t ordinary_slot_count,
    std::uint32_t slot_limit,
    std::uint32_t promotion_threshold,
    std::uint32_t consumes_special_draw,
    std::uint32_t minimum_roll_percent,
    std::uint32_t maximum_roll_percent,
    std::uint32_t apply_r4_finalizer,
    std::uint32_t auxiliary_mode_threshold,
    std::uint32_t preferred_vendor_id,
    std::uint32_t* output_masks,
    std::uint32_t* output_vendor_id) {
    if (
        seeds == nullptr || seed_count == 0u || seed_count > 1000000u ||
        candidates == nullptr || candidate_count == 0u ||
        candidate_count > 4096u || special_groups == nullptr ||
        (special_group_count != 1u && special_group_count != 65536u) ||
        category_capacities == nullptr || criterion_keys == nullptr ||
        criterion_key_count == 0u || criterion_offsets == nullptr ||
        criterion_kinds == nullptr || criterion_group_count == 0u ||
        criterion_group_count > 32u || criterion_offsets[0] != 0u ||
        criterion_offsets[criterion_group_count] != criterion_key_count ||
        rarity < 3u || rarity > 5u || ordinary_slot_count < 4u ||
        ordinary_slot_count > 5u || slot_limit < ordinary_slot_count ||
        slot_limit > 6u || promotion_threshold > 10000u ||
        consumes_special_draw > 1u || minimum_roll_percent > 100u ||
        maximum_roll_percent > 100u ||
        minimum_roll_percent > maximum_roll_percent ||
        apply_r4_finalizer > 1u ||
        (apply_r4_finalizer != 0u &&
            (rarity != 4u || ordinary_slot_count != 4u ||
             consumes_special_draw == 0u || special_group_count != 65536u)) ||
        auxiliary_mode_threshold > 10000u || output_masks == nullptr ||
        output_vendor_id == nullptr) {
        return -1;
    }
    for (std::uint32_t group = 0u; group < criterion_group_count; ++group) {
        if (criterion_offsets[group] >= criterion_offsets[group + 1u] ||
            criterion_kinds[group] > 2u) {
            return -1;
        }
    }

    DeviceBundle bundle;
    if (!create_device(preferred_vendor_id, &bundle)) return -2;
    *output_vendor_id = bundle.vendor_id;

    ComPtr<ID3D11ComputeShader> shader;
    if (FAILED(bundle.device->CreateComputeShader(
            g_effect_filter_shader_bytecode,
            sizeof(g_effect_filter_shader_bytecode),
            nullptr,
            &shader))) {
        return -3;
    }

    ComPtr<ID3D11Buffer> seed_buffer;
    ComPtr<ID3D11ShaderResourceView> seed_view;
    ComPtr<ID3D11Buffer> candidate_buffer;
    ComPtr<ID3D11ShaderResourceView> candidate_view;
    ComPtr<ID3D11Buffer> special_buffer;
    ComPtr<ID3D11ShaderResourceView> special_view;
    ComPtr<ID3D11Buffer> capacity_buffer;
    ComPtr<ID3D11ShaderResourceView> capacity_view;
    ComPtr<ID3D11Buffer> key_buffer;
    ComPtr<ID3D11ShaderResourceView> key_view;
    ComPtr<ID3D11Buffer> offset_buffer;
    ComPtr<ID3D11ShaderResourceView> offset_view;
    ComPtr<ID3D11Buffer> kind_buffer;
    ComPtr<ID3D11ShaderResourceView> kind_view;
    if (
        !create_structured_buffer(
            bundle.device.Get(), seeds, seed_count, &seed_buffer, &seed_view) ||
        !create_structured_buffer(
            bundle.device.Get(), candidates, candidate_count,
            &candidate_buffer, &candidate_view) ||
        !create_structured_buffer(
            bundle.device.Get(), special_groups, special_group_count,
            &special_buffer, &special_view) ||
        !create_structured_buffer(
            bundle.device.Get(), category_capacities, 32u,
            &capacity_buffer, &capacity_view) ||
        !create_structured_buffer(
            bundle.device.Get(), criterion_keys, criterion_key_count,
            &key_buffer, &key_view) ||
        !create_structured_buffer(
            bundle.device.Get(), criterion_offsets, criterion_group_count + 1u,
            &offset_buffer, &offset_view) ||
        !create_structured_buffer(
            bundle.device.Get(), criterion_kinds, criterion_group_count,
            &kind_buffer, &kind_view)) {
        return -4;
    }

    ComPtr<ID3D11Buffer> output_buffer;
    ComPtr<ID3D11UnorderedAccessView> output_view;
    if (!create_output_u32_buffer(
            bundle.device.Get(), seed_count, &output_buffer, &output_view)) {
        return -4;
    }

    EffectFilterConstants constants{};
    constants.seed_count = seed_count;
    constants.candidate_count = candidate_count;
    constants.criterion_group_count = criterion_group_count;
    constants.rarity = rarity;
    constants.ordinary_slot_count = ordinary_slot_count;
    constants.slot_limit = slot_limit;
    constants.promotion_threshold = promotion_threshold;
    constants.consumes_special_draw = consumes_special_draw;
    constants.minimum_roll_percent = minimum_roll_percent;
    constants.maximum_roll_percent = maximum_roll_percent;
    constants.apply_r4_finalizer = apply_r4_finalizer;
    constants.auxiliary_mode_threshold = auxiliary_mode_threshold;
    D3D11_BUFFER_DESC constant_description{};
    constant_description.ByteWidth = sizeof(constants);
    constant_description.Usage = D3D11_USAGE_IMMUTABLE;
    constant_description.BindFlags = D3D11_BIND_CONSTANT_BUFFER;
    D3D11_SUBRESOURCE_DATA constant_initial{};
    constant_initial.pSysMem = &constants;
    ComPtr<ID3D11Buffer> constant_buffer;
    if (FAILED(bundle.device->CreateBuffer(
            &constant_description,
            &constant_initial,
            &constant_buffer))) {
        return -4;
    }

    ID3D11ShaderResourceView* shader_views[] = {
        seed_view.Get(),
        candidate_view.Get(),
        special_view.Get(),
        capacity_view.Get(),
        key_view.Get(),
        offset_view.Get(),
        kind_view.Get(),
    };
    ID3D11UnorderedAccessView* unordered_views[] = {output_view.Get()};
    bundle.context->CSSetShader(shader.Get(), nullptr, 0u);
    bundle.context->CSSetShaderResources(0u, ARRAYSIZE(shader_views), shader_views);
    bundle.context->CSSetUnorderedAccessViews(
        0u, ARRAYSIZE(unordered_views), unordered_views, nullptr);
    bundle.context->CSSetConstantBuffers(0u, 1u, constant_buffer.GetAddressOf());
    const std::uint32_t groups = (seed_count + 127u) / 128u;
    bundle.context->Dispatch(groups, 1u, 1u);
    bundle.context->Flush();
    if (!read_buffer(
            bundle.device.Get(),
            bundle.context.Get(),
            output_buffer.Get(),
            output_masks,
            seed_count * sizeof(std::uint32_t))) {
        return -5;
    }
    return 1;
}
