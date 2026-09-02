#include <cstddef>
#include <cstdint>

#include <cuda_runtime.h>

#pragma pack(push, 1)
struct SpecialRuleInput {
    std::uint16_t key;
    std::uint16_t weight;
    float cost;
    std::uint16_t conflict_identity_0;
    std::uint16_t conflict_identity_1;
    std::uint8_t active_conflict_mask;
    std::uint8_t scratch_bit;
    std::uint16_t reserved;
};
#pragma pack(pop)

static_assert(sizeof(SpecialRuleInput) == 16u, "special-rule ABI changed");

namespace {

constexpr std::uint32_t kLcgInverse = 0xA5E2A705u;
constexpr std::uint32_t kLcgMultiplier = 0x00010DCDu;
constexpr std::uint32_t kAuxiliaryModeSeedMaskLow = 0x01E3C78Fu;
constexpr std::uint32_t kAuxiliaryModeSeedMaskHigh = 0x00E1C387u;
int g_last_backend = -1;

__host__ __device__ std::uint32_t lcg_step(std::uint32_t state) {
    return kLcgMultiplier * state + 1u;
}

__host__ __device__ bool is_natural_seed(std::uint32_t seed) {
    return (seed & 0xF0000000u) == 0u && (seed & 0xFFFFu) != 0u;
}

__host__ __device__ std::uint32_t derive_auxiliary_mode_seed(
    std::uint32_t displayed_seed) {
    return ((displayed_seed & kAuxiliaryModeSeedMaskLow) << 3u) |
        ((displayed_seed >> 4u) & kAuxiliaryModeSeedMaskHigh);
}

__host__ __device__ std::uint32_t derive_terrain_seed(
    std::uint32_t displayed_seed) {
    return ((displayed_seed >> 14u) & 0x3FFFu) |
        ((displayed_seed & 0x3FFFu) << 14u);
}

__host__ __device__ std::uint32_t random_index_from_u16(
    std::uint16_t value,
    std::uint32_t count) {
    return static_cast<std::uint32_t>(
        (static_cast<std::uint64_t>(value) * count) >> 16u);
}

__host__ __device__ int roll_10000_from_u16(std::uint16_t value) {
#if defined(__CUDA_ARCH__)
    const float unit = __fmul_rn(__uint2float_rn(value), 1.0f / 65536.0f);
    const float product = __fmul_rn(unit, 10000.0f);
    const int result = __float2int_rz(product);
#else
    // The game rounds both multiplications to float32 before truncation.
    // Volatile prevents the host compiler from contracting the operations.
    volatile float unit = static_cast<float>(value) * (1.0f / 65536.0f);
    volatile float product = unit * 10000.0f;
    const int result = static_cast<int>(product);
#endif
    return result < 9999 ? result : 9999;
}

__host__ __device__ std::uint32_t generate_terrain_row_index(
    std::uint32_t displayed_seed,
    int mode_threshold,
    const std::uint32_t* filtered_rows,
    std::uint32_t filtered_count,
    std::uint32_t terrain_row_count) {
    std::uint32_t mode_state = derive_auxiliary_mode_seed(displayed_seed);
    mode_state = lcg_step(mode_state);
    const int first_roll = roll_10000_from_u16(
        static_cast<std::uint16_t>(mode_state >> 16u));
    std::uint32_t branch_class = 2u;
    if (first_roll >= mode_threshold) {
        mode_state = lcg_step(mode_state);
        const std::uint32_t split = random_index_from_u16(
            static_cast<std::uint16_t>(mode_state >> 16u), 2u);
        branch_class = split == 0u ? 1u : 0u;
    }

    std::uint32_t terrain_state = lcg_step(derive_terrain_seed(displayed_seed));
    const std::uint16_t terrain_draw = static_cast<std::uint16_t>(
        terrain_state >> 16u);
    if (branch_class == 2u) {
        return random_index_from_u16(terrain_draw, terrain_row_count);
    }
    return filtered_rows[random_index_from_u16(terrain_draw, filtered_count)];
}

#include "native_enemy_matcher.cuh"

__global__ void collect_natural_pivot_seeds_kernel(
    const std::uint16_t* values,
    std::uint32_t value_count,
    std::uint64_t start_index,
    std::uint64_t item_count,
    std::uint16_t low16_stride,
    std::uint32_t draw_index,
    std::uint32_t* output_seeds,
    std::uint64_t* output_trials,
    unsigned long long* output_count,
    std::uint64_t output_capacity) {
    const std::uint64_t item_index =
        static_cast<std::uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (item_index >= item_count) {
        return;
    }
    const std::uint64_t flat_index = start_index + item_index;
    const std::uint64_t low_index = flat_index / value_count;
    const std::uint32_t bucket_index =
        static_cast<std::uint32_t>(flat_index % value_count);
    const std::uint16_t low16 = static_cast<std::uint16_t>(
        static_cast<std::uint32_t>(low_index) * low16_stride);
    const std::uint32_t rotation =
        static_cast<std::uint32_t>(low_index % value_count);
    const std::uint16_t high16 =
        values[(rotation + bucket_index) % value_count];
    const std::uint32_t state =
        (static_cast<std::uint32_t>(high16) << 16u) | low16;
    std::uint32_t seed = state;
    for (std::uint32_t draw = 0u; draw < draw_index; ++draw) {
        seed = kLcgInverse * (seed - 1u);
    }
    if (!is_natural_seed(seed)) {
        return;
    }
    const unsigned long long output_index = atomicAdd(output_count, 1ull);
    if (output_index < output_capacity) {
        output_seeds[output_index] = seed;
        output_trials[output_index] = flat_index + 1u;
    }
}

__global__ void generate_ng3_primary_ids_kernel(
    const std::uint32_t* seeds,
    std::uint64_t count,
    const std::uint32_t* normal_lookup,
    const std::uint32_t* promoted_lookup,
    const std::uint8_t* promotion_success_lookup,
    const std::uint8_t* random7_lookup,
    std::uint32_t* output_effect_ids) {
    const std::uint64_t index =
        static_cast<std::uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index >= count) {
        return;
    }
    std::uint32_t state = lcg_step(seeds[index]);  // Grace draw.
    state = lcg_step(state);                       // Promotion trial.
    const bool promoted = promotion_success_lookup[state >> 16u] != 0u;
    bool primary_promoted = false;
    if (promoted) {
        std::uint8_t order[7] = {0, 1, 2, 3, 4, 5, 6};
        for (std::uint32_t position = 0u; position < 7u; ++position) {
            state = lcg_step(state);
            const std::uint8_t swap_index = random7_lookup[state >> 16u];
            const std::uint8_t temporary = order[position];
            order[position] = order[swap_index];
            order[swap_index] = temporary;
        }
        for (std::uint32_t position = 0u; position < 7u; ++position) {
            const std::uint8_t selected = order[position];
            if (selected > 0u && selected < 6u) {
                primary_promoted = selected == 1u;
                break;
            }
        }
    }
    state = lcg_step(state);  // Primary weighted lottery.
    const std::uint16_t lottery_u16 = static_cast<std::uint16_t>(state >> 16u);
    output_effect_ids[index] =
        primary_promoted ? promoted_lookup[lottery_u16] : normal_lookup[lottery_u16];
}

__global__ void generate_ng3_primary_ids_context_kernel(
    const std::uint32_t* seeds,
    std::uint64_t count,
    const std::uint32_t* normal_lookup,
    const std::uint32_t* promoted_lookup,
    const std::uint8_t* promotion_success_lookup,
    const std::uint8_t* random7_lookup,
    std::uint32_t pre_promotion_draws,
    std::uint8_t slot_limit,
    std::uint8_t excluded_slot_mask,
    std::uint8_t primary_source_index,
    std::uint32_t* output_effect_ids) {
    const std::uint64_t index =
        static_cast<std::uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index >= count) {
        return;
    }
    std::uint32_t state = seeds[index];
    for (std::uint32_t draw = 0u; draw < pre_promotion_draws; ++draw) {
        state = lcg_step(state);
    }
    state = lcg_step(state);
    const bool promoted = promotion_success_lookup[state >> 16u] != 0u;
    bool primary_promoted = false;
    if (promoted) {
        std::uint8_t order[7] = {0, 1, 2, 3, 4, 5, 6};
        for (std::uint32_t position = 0u; position < 7u; ++position) {
            state = lcg_step(state);
            const std::uint8_t swap_index = random7_lookup[state >> 16u];
            const std::uint8_t temporary = order[position];
            order[position] = order[swap_index];
            order[swap_index] = temporary;
        }
        for (std::uint8_t selected : order) {
            if (selected >= slot_limit ||
                (excluded_slot_mask & (1u << selected)) != 0u) {
                continue;
            }
            primary_promoted = selected == primary_source_index;
            break;
        }
    }
    state = lcg_step(state);
    const std::uint16_t lottery_u16 = static_cast<std::uint16_t>(state >> 16u);
    output_effect_ids[index] =
        primary_promoted ? promoted_lookup[lottery_u16] : normal_lookup[lottery_u16];
}

__global__ void generate_ng3_r4_primary_ids_multi_kernel(
    const std::uint32_t* seeds,
    std::uint64_t count,
    const std::uint8_t* context_by_first_u16,
    const std::uint32_t* normal_lookups,
    const std::uint32_t* promoted_lookups,
    const std::uint8_t* promotion_success_lookup,
    const std::uint8_t* random7_lookup,
    std::uint32_t* output_effect_ids) {
    const std::uint64_t index =
        static_cast<std::uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index >= count) {
        return;
    }
    std::uint32_t state = lcg_step(seeds[index]);
    const std::uint32_t context = context_by_first_u16[state >> 16u];
    state = lcg_step(state);
    const bool promoted = promotion_success_lookup[state >> 16u] != 0u;
    bool primary_promoted = false;
    if (promoted) {
        std::uint8_t order[7] = {0, 1, 2, 3, 4, 5, 6};
        for (std::uint32_t position = 0u; position < 7u; ++position) {
            state = lcg_step(state);
            const std::uint8_t swap_index = random7_lookup[state >> 16u];
            const std::uint8_t temporary = order[position];
            order[position] = order[swap_index];
            order[swap_index] = temporary;
        }
        for (std::uint8_t selected : order) {
            if (selected >= 5u || selected == 0u) {
                continue;
            }
            primary_promoted = selected == 1u;
            break;
        }
    }
    state = lcg_step(state);
    const std::uint32_t lookup_index = context * 65536u + (state >> 16u);
    output_effect_ids[index] = primary_promoted
        ? promoted_lookups[lookup_index]
        : normal_lookups[lookup_index];
}

__global__ void collect_ng3_r4_primary_pivot_seeds_kernel(
    const std::uint16_t* values,
    std::uint32_t value_count,
    std::uint64_t start_index,
    std::uint64_t item_count,
    std::uint16_t low16_stride,
    const std::uint32_t* allowed_effect_ids,
    std::uint32_t allowed_effect_count,
    const std::uint8_t* context_by_first_u16,
    const std::uint32_t* normal_lookups,
    const std::uint32_t* promoted_lookups,
    const std::uint8_t* promotion_success_lookup,
    const std::uint8_t* random7_lookup,
    std::uint32_t* output_seeds,
    std::uint64_t* output_trials,
    unsigned long long* output_count,
    std::uint64_t output_capacity) {
    const std::uint64_t item_index =
        static_cast<std::uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (item_index >= item_count) {
        return;
    }
    const std::uint64_t flat_index = start_index + item_index;
    const std::uint64_t low_index = flat_index / value_count;
    const std::uint32_t bucket_index =
        static_cast<std::uint32_t>(flat_index % value_count);
    const std::uint16_t low16 = static_cast<std::uint16_t>(
        static_cast<std::uint32_t>(low_index) * low16_stride);
    const std::uint32_t rotation =
        static_cast<std::uint32_t>(low_index % value_count);
    const std::uint16_t high16 = values[(rotation + bucket_index) % value_count];
    const std::uint32_t pivot_state =
        (static_cast<std::uint32_t>(high16) << 16u) | low16;
    const std::uint32_t seed = kLcgInverse * (pivot_state - 1u);
    if (!is_natural_seed(seed)) {
        return;
    }

    std::uint32_t state = lcg_step(seed);
    const std::uint32_t context = context_by_first_u16[state >> 16u];
    state = lcg_step(state);
    const bool promoted = promotion_success_lookup[state >> 16u] != 0u;
    bool primary_promoted = false;
    if (promoted) {
        std::uint8_t order[7] = {0, 1, 2, 3, 4, 5, 6};
        for (std::uint32_t position = 0u; position < 7u; ++position) {
            state = lcg_step(state);
            const std::uint8_t swap_index = random7_lookup[state >> 16u];
            const std::uint8_t temporary = order[position];
            order[position] = order[swap_index];
            order[swap_index] = temporary;
        }
        for (std::uint8_t selected : order) {
            if (selected >= 5u || selected == 0u) {
                continue;
            }
            primary_promoted = selected == 1u;
            break;
        }
    }
    state = lcg_step(state);
    const std::uint32_t lookup_index = context * 65536u + (state >> 16u);
    const std::uint32_t effect_id = primary_promoted
        ? promoted_lookups[lookup_index]
        : normal_lookups[lookup_index];
    bool matches = false;
    for (std::uint32_t index = 0u; index < allowed_effect_count; ++index) {
        if (effect_id == allowed_effect_ids[index]) {
            matches = true;
            break;
        }
    }
    if (!matches) {
        return;
    }
    const unsigned long long output_index = atomicAdd(output_count, 1ull);
    if (output_index < output_capacity) {
        output_seeds[output_index] = seed;
        output_trials[output_index] = flat_index + 1u;
    }
}

__global__ void generate_terrain_row_indices_kernel(
    const std::uint32_t* seeds,
    std::uint64_t count,
    int mode_threshold,
    const std::uint32_t* filtered_rows,
    std::uint32_t filtered_count,
    std::uint32_t terrain_row_count,
    std::uint32_t* output_rows) {
    const std::uint64_t index =
        static_cast<std::uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index >= count) {
        return;
    }
    output_rows[index] = generate_terrain_row_index(
        seeds[index],
        mode_threshold,
        filtered_rows,
        filtered_count,
        terrain_row_count);
}

__global__ void match_enemy_constraints_kernel(
    const std::uint32_t* seeds,
    const std::uint32_t* terrain_rows,
    std::uint64_t count,
    std::uint8_t playthrough,
    int mode_threshold,
    const int* descriptor_thresholds,
    int selector_threshold,
    int role_five_threshold,
    std::uint8_t selector_value,
    const EnemyCandidateInput* enemy_rows,
    std::uint32_t enemy_row_count,
    const EnemyTerrainInput* terrains,
    std::uint32_t terrain_count,
    const EnemyContextInput* contexts,
    std::uint32_t context_count,
    const std::uint32_t* criterion_keys,
    const std::uint16_t* group_offsets,
    std::uint32_t group_count,
    std::uint32_t* output_masks) {
    const std::uint64_t index =
        static_cast<std::uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index >= count) {
        return;
    }
    std::uint32_t observed_mask = 0u;
    native_enemy_matcher::match_enemy_constraints_for_seed(
        seeds[index],
        terrain_rows[index],
        playthrough,
        mode_threshold,
        descriptor_thresholds,
        selector_threshold,
        role_five_threshold,
        selector_value,
        enemy_rows,
        enemy_row_count,
        terrains,
        terrain_count,
        contexts,
        context_count,
        criterion_keys,
        group_offsets,
        group_count,
        &observed_mask);
    output_masks[index] = observed_mask;
}

__host__ __device__ bool special_rule_conflicts(
    const SpecialRuleInput& current,
    const SpecialRuleInput& previous) {
    const std::uint16_t current_identities[2] = {
        current.conflict_identity_0,
        current.conflict_identity_1,
    };
    const std::uint16_t previous_identities[2] = {
        previous.conflict_identity_0,
        previous.conflict_identity_1,
    };
    for (std::uint32_t current_index = 0u; current_index < 2u; ++current_index) {
        if ((current.active_conflict_mask & (1u << current_index)) == 0u) {
            continue;
        }
        const std::uint16_t identity = current_identities[current_index];
        if (identity == 0xFFFFu) {
            continue;
        }
        for (std::uint32_t previous_index = 0u; previous_index < 2u;
             ++previous_index) {
            if (identity == previous_identities[previous_index]) {
                return true;
            }
        }
    }
    return false;
}

__host__ __device__ void mark_special_rule_constraints(
    std::uint16_t key,
    const std::uint16_t* criterion_keys,
    const std::uint16_t* group_offsets,
    std::uint32_t group_count,
    std::uint32_t& matched_mask) {
    for (std::uint32_t group = 0u; group < group_count; ++group) {
        if ((matched_mask & (1u << group)) != 0u) {
            continue;
        }
        for (std::uint32_t index = group_offsets[group];
             index < group_offsets[group + 1u]; ++index) {
            if (criterion_keys[index] == key) {
                matched_mask |= 1u << group;
                break;
            }
        }
    }
}

__host__ __device__ std::uint32_t match_special_rules_for_seed(
    std::uint32_t displayed_seed,
    std::uint32_t scratch_mask,
    const SpecialRuleInput* rows,
    std::uint32_t row_count,
    const std::uint16_t* criterion_keys,
    const std::uint16_t* group_offsets,
    std::uint32_t group_count) {
    std::uint32_t state = displayed_seed & 0x0FFFFFFFu;
    state = lcg_step(state);
    const int target_budget = static_cast<int>(
        random_index_from_u16(static_cast<std::uint16_t>(state >> 16u), 5u)) + 1;
    float remaining = static_cast<float>(target_budget);
    const float original_budget = remaining;
    std::uint16_t selected_keys[3] = {};
    std::uint16_t selected_rows[3] = {};
    std::uint32_t selected_count = 0u;
    bool zero_selected = false;
    float third_slot_best_abs = 0.0f;
    std::uint32_t matched_mask = 0u;

    for (std::uint32_t attempt = 0u; attempt < 3u; ++attempt) {
        std::uint16_t total_weight = 0u;
        for (std::uint32_t index = 0u; index < row_count; ++index) {
            const SpecialRuleInput& row = rows[index];
            if (row.weight == 0u ||
                (row.scratch_bit < 32u &&
                 (scratch_mask & (1u << row.scratch_bit)) != 0u)) {
                continue;
            }
            bool accepted = true;
            if (selected_count != 0u) {
                if (row.key == 0u) {
                    accepted = !zero_selected;
                } else {
                    for (std::uint32_t selected = 0u;
                         selected < selected_count; ++selected) {
                        if (row.key == selected_keys[selected] ||
                            special_rule_conflicts(row, rows[selected_rows[selected]])) {
                            accepted = false;
                            break;
                        }
                    }
                }
            }
            if (!accepted) {
                continue;
            }
            if (selected_count == 1u) {
                const float accumulated_delta = remaining - original_budget;
                if ((accumulated_delta < 0.0f && row.cost >= 0.0f) ||
                    (accumulated_delta > 0.0f && row.cost <= 0.0f)) {
                    continue;
                }
            } else if (selected_count == 2u) {
                if ((remaining < 0.0f && row.cost > 0.0f) ||
                    (remaining > 0.0f && row.cost < 0.0f)) {
                    continue;
                }
                const float absolute_cost = row.cost < 0.0f ? -row.cost : row.cost;
                const float absolute_remaining =
                    remaining < 0.0f ? -remaining : remaining;
                if (absolute_cost > absolute_remaining ||
                    absolute_cost < third_slot_best_abs) {
                    continue;
                }
                if (absolute_cost > third_slot_best_abs) {
                    total_weight = 0u;
                    third_slot_best_abs = absolute_cost;
                }
            }
            total_weight = static_cast<std::uint16_t>(
                total_weight + row.weight);
        }
        if (total_weight == 0u) {
            break;
        }

        state = lcg_step(state);
        std::uint32_t ticket = random_index_from_u16(
            static_cast<std::uint16_t>(state >> 16u), total_weight);
        int chosen_index = -1;
        for (std::uint32_t index = 0u; index < row_count; ++index) {
            const SpecialRuleInput& row = rows[index];
            if (row.weight == 0u ||
                (row.scratch_bit < 32u &&
                 (scratch_mask & (1u << row.scratch_bit)) != 0u)) {
                continue;
            }
            bool accepted = true;
            if (selected_count != 0u) {
                if (row.key == 0u) {
                    accepted = !zero_selected;
                } else {
                    for (std::uint32_t selected = 0u;
                         selected < selected_count; ++selected) {
                        if (row.key == selected_keys[selected] ||
                            special_rule_conflicts(row, rows[selected_rows[selected]])) {
                            accepted = false;
                            break;
                        }
                    }
                }
            }
            if (!accepted) {
                continue;
            }
            if (selected_count == 1u) {
                const float accumulated_delta = remaining - original_budget;
                if ((accumulated_delta < 0.0f && row.cost >= 0.0f) ||
                    (accumulated_delta > 0.0f && row.cost <= 0.0f)) {
                    continue;
                }
            } else if (selected_count == 2u) {
                if ((remaining < 0.0f && row.cost > 0.0f) ||
                    (remaining > 0.0f && row.cost < 0.0f)) {
                    continue;
                }
                const float absolute_cost = row.cost < 0.0f ? -row.cost : row.cost;
                const float absolute_remaining =
                    remaining < 0.0f ? -remaining : remaining;
                if (absolute_cost > absolute_remaining ||
                    absolute_cost != third_slot_best_abs) {
                    continue;
                }
            }
            if (ticket < row.weight) {
                chosen_index = static_cast<int>(index);
                break;
            }
            ticket -= row.weight;
        }
        if (chosen_index < 0) {
            break;
        }
        const SpecialRuleInput& chosen = rows[chosen_index];
        selected_keys[selected_count] = chosen.key;
        selected_rows[selected_count] = static_cast<std::uint16_t>(chosen_index);
        ++selected_count;
        if (chosen.key == 0u) {
            zero_selected = true;
        } else {
            mark_special_rule_constraints(
                chosen.key,
                criterion_keys,
                group_offsets,
                group_count,
                matched_mask);
        }
        remaining = remaining - chosen.cost;
        if (remaining == 0.0f) {
            break;
        }
    }
    return matched_mask;
}

__global__ void match_special_rule_constraints_kernel(
    const std::uint32_t* seeds,
    const std::uint32_t* scratch_masks,
    std::uint64_t count,
    const SpecialRuleInput* rows,
    std::uint32_t row_count,
    const std::uint16_t* criterion_keys,
    const std::uint16_t* group_offsets,
    std::uint32_t group_count,
    std::uint32_t* output_masks) {
    const std::uint64_t index =
        static_cast<std::uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index >= count) {
        return;
    }
    output_masks[index] = match_special_rules_for_seed(
        seeds[index],
        scratch_masks[index],
        rows,
        row_count,
        criterion_keys,
        group_offsets,
        group_count);
}

std::uint64_t collect_on_cpu(
    const std::uint16_t* values,
    std::uint32_t value_count,
    std::uint64_t start_index,
    std::uint64_t stop_index,
    std::uint16_t low16_stride,
    std::uint32_t draw_index,
    std::uint32_t* output_seeds,
    std::uint64_t* output_trials,
    std::uint64_t output_capacity) {
    std::uint64_t output_count = 0u;
    for (std::uint64_t flat_index = start_index; flat_index < stop_index;
         ++flat_index) {
        const std::uint64_t low_index = flat_index / value_count;
        const std::uint32_t bucket_index =
            static_cast<std::uint32_t>(flat_index % value_count);
        const std::uint16_t low16 = static_cast<std::uint16_t>(
            static_cast<std::uint32_t>(low_index) * low16_stride);
        const std::uint32_t rotation =
            static_cast<std::uint32_t>(low_index % value_count);
        const std::uint16_t high16 =
            values[(rotation + bucket_index) % value_count];
        const std::uint32_t state =
            (static_cast<std::uint32_t>(high16) << 16u) | low16;
        std::uint32_t seed = state;
        for (std::uint32_t draw = 0u; draw < draw_index; ++draw) {
            seed = kLcgInverse * (seed - 1u);
        }
        if (!is_natural_seed(seed)) {
            continue;
        }
        if (output_count >= output_capacity || output_seeds == nullptr ||
            output_trials == nullptr) {
            return UINT64_MAX;
        }
        output_seeds[output_count] = seed;
        output_trials[output_count] = flat_index + 1u;
        ++output_count;
    }
    return output_count;
}

std::uint64_t collect_on_cuda(
    const std::uint16_t* values,
    std::uint32_t value_count,
    std::uint64_t start_index,
    std::uint64_t stop_index,
    std::uint16_t low16_stride,
    std::uint32_t draw_index,
    std::uint32_t* output_seeds,
    std::uint64_t* output_trials,
    std::uint64_t output_capacity) {
    const std::uint64_t item_count = stop_index - start_index;
    std::uint16_t* device_values = nullptr;
    std::uint32_t* device_seeds = nullptr;
    std::uint64_t* device_trials = nullptr;
    unsigned long long* device_count = nullptr;
    auto cleanup = [&]() {
        cudaFree(device_values);
        cudaFree(device_seeds);
        cudaFree(device_trials);
        cudaFree(device_count);
    };
    if (cudaMalloc(&device_values, value_count * sizeof(std::uint16_t)) != cudaSuccess ||
        cudaMalloc(&device_seeds, output_capacity * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_trials, output_capacity * sizeof(std::uint64_t)) != cudaSuccess ||
        cudaMalloc(&device_count, sizeof(unsigned long long)) != cudaSuccess) {
        cleanup();
        return UINT64_MAX;
    }
    unsigned long long zero = 0u;
    if (cudaMemcpy(
            device_values,
            values,
            value_count * sizeof(std::uint16_t),
            cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_count, &zero, sizeof(zero), cudaMemcpyHostToDevice) != cudaSuccess) {
        cleanup();
        return UINT64_MAX;
    }
    constexpr std::uint32_t threads = 256u;
    const std::uint32_t blocks = static_cast<std::uint32_t>(
        (item_count + threads - 1u) / threads);
    collect_natural_pivot_seeds_kernel<<<blocks, threads>>>(
        device_values,
        value_count,
        start_index,
        item_count,
        low16_stride,
        draw_index,
        device_seeds,
        device_trials,
        device_count,
        output_capacity);
    unsigned long long count = 0u;
    if (cudaGetLastError() != cudaSuccess || cudaDeviceSynchronize() != cudaSuccess ||
        cudaMemcpy(&count, device_count, sizeof(count), cudaMemcpyDeviceToHost) != cudaSuccess ||
        count > output_capacity ||
        cudaMemcpy(
            output_seeds,
            device_seeds,
            count * sizeof(std::uint32_t),
            cudaMemcpyDeviceToHost) != cudaSuccess ||
        cudaMemcpy(
            output_trials,
            device_trials,
            count * sizeof(std::uint64_t),
            cudaMemcpyDeviceToHost) != cudaSuccess) {
        cleanup();
        return UINT64_MAX;
    }
    cleanup();
    return static_cast<std::uint64_t>(count);
}

bool generate_primary_ids_on_cuda(
    const std::uint32_t* seeds,
    std::uint64_t count,
    const std::uint32_t* normal_lookup,
    const std::uint32_t* promoted_lookup,
    const std::uint8_t* promotion_success_lookup,
    const std::uint8_t* random7_lookup,
    std::uint32_t* output_effect_ids) {
    std::uint32_t* device_seeds = nullptr;
    std::uint32_t* device_normal = nullptr;
    std::uint32_t* device_promoted = nullptr;
    std::uint8_t* device_promotion = nullptr;
    std::uint8_t* device_random7 = nullptr;
    std::uint32_t* device_output = nullptr;
    auto cleanup = [&]() {
        cudaFree(device_seeds);
        cudaFree(device_normal);
        cudaFree(device_promoted);
        cudaFree(device_promotion);
        cudaFree(device_random7);
        cudaFree(device_output);
    };
    constexpr std::size_t u16_count = 65536u;
    if (cudaMalloc(&device_seeds, count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_normal, u16_count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_promoted, u16_count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_promotion, u16_count * sizeof(std::uint8_t)) != cudaSuccess ||
        cudaMalloc(&device_random7, u16_count * sizeof(std::uint8_t)) != cudaSuccess ||
        cudaMalloc(&device_output, count * sizeof(std::uint32_t)) != cudaSuccess) {
        cleanup();
        return false;
    }
    if (cudaMemcpy(device_seeds, seeds, count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_normal, normal_lookup, u16_count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_promoted, promoted_lookup, u16_count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_promotion, promotion_success_lookup, u16_count, cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_random7, random7_lookup, u16_count, cudaMemcpyHostToDevice) != cudaSuccess) {
        cleanup();
        return false;
    }
    constexpr std::uint32_t threads = 256u;
    const std::uint32_t blocks = static_cast<std::uint32_t>((count + threads - 1u) / threads);
    generate_ng3_primary_ids_kernel<<<blocks, threads>>>(
        device_seeds,
        count,
        device_normal,
        device_promoted,
        device_promotion,
        device_random7,
        device_output);
    const bool success =
        cudaGetLastError() == cudaSuccess &&
        cudaDeviceSynchronize() == cudaSuccess &&
        cudaMemcpy(
            output_effect_ids,
            device_output,
            count * sizeof(std::uint32_t),
            cudaMemcpyDeviceToHost) == cudaSuccess;
    cleanup();
    return success;
}

bool generate_primary_ids_context_on_cuda(
    const std::uint32_t* seeds,
    std::uint64_t count,
    const std::uint32_t* normal_lookup,
    const std::uint32_t* promoted_lookup,
    const std::uint8_t* promotion_success_lookup,
    const std::uint8_t* random7_lookup,
    std::uint32_t pre_promotion_draws,
    std::uint8_t slot_limit,
    std::uint8_t excluded_slot_mask,
    std::uint8_t primary_source_index,
    std::uint32_t* output_effect_ids) {
    std::uint32_t* device_seeds = nullptr;
    std::uint32_t* device_normal = nullptr;
    std::uint32_t* device_promoted = nullptr;
    std::uint8_t* device_promotion = nullptr;
    std::uint8_t* device_random7 = nullptr;
    std::uint32_t* device_output = nullptr;
    auto cleanup = [&]() {
        cudaFree(device_seeds);
        cudaFree(device_normal);
        cudaFree(device_promoted);
        cudaFree(device_promotion);
        cudaFree(device_random7);
        cudaFree(device_output);
    };
    constexpr std::size_t u16_count = 65536u;
    if (cudaMalloc(&device_seeds, count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_normal, u16_count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_promoted, u16_count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_promotion, u16_count * sizeof(std::uint8_t)) != cudaSuccess ||
        cudaMalloc(&device_random7, u16_count * sizeof(std::uint8_t)) != cudaSuccess ||
        cudaMalloc(&device_output, count * sizeof(std::uint32_t)) != cudaSuccess) {
        cleanup();
        return false;
    }
    if (cudaMemcpy(device_seeds, seeds, count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_normal, normal_lookup, u16_count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_promoted, promoted_lookup, u16_count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_promotion, promotion_success_lookup, u16_count, cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_random7, random7_lookup, u16_count, cudaMemcpyHostToDevice) != cudaSuccess) {
        cleanup();
        return false;
    }
    constexpr std::uint32_t threads = 256u;
    const std::uint32_t blocks = static_cast<std::uint32_t>((count + threads - 1u) / threads);
    generate_ng3_primary_ids_context_kernel<<<blocks, threads>>>(
        device_seeds,
        count,
        device_normal,
        device_promoted,
        device_promotion,
        device_random7,
        pre_promotion_draws,
        slot_limit,
        excluded_slot_mask,
        primary_source_index,
        device_output);
    const bool success =
        cudaGetLastError() == cudaSuccess &&
        cudaDeviceSynchronize() == cudaSuccess &&
        cudaMemcpy(
            output_effect_ids,
            device_output,
            count * sizeof(std::uint32_t),
            cudaMemcpyDeviceToHost) == cudaSuccess;
    cleanup();
    return success;
}

bool generate_r4_primary_ids_multi_on_cuda(
    const std::uint32_t* seeds,
    std::uint64_t count,
    const std::uint8_t* context_by_first_u16,
    std::uint32_t context_count,
    const std::uint32_t* normal_lookups,
    const std::uint32_t* promoted_lookups,
    const std::uint8_t* promotion_success_lookup,
    const std::uint8_t* random7_lookup,
    std::uint32_t* output_effect_ids) {
    std::uint32_t* device_seeds = nullptr;
    std::uint8_t* device_contexts = nullptr;
    std::uint32_t* device_normal = nullptr;
    std::uint32_t* device_promoted = nullptr;
    std::uint8_t* device_promotion = nullptr;
    std::uint8_t* device_random7 = nullptr;
    std::uint32_t* device_output = nullptr;
    auto cleanup = [&]() {
        cudaFree(device_seeds);
        cudaFree(device_contexts);
        cudaFree(device_normal);
        cudaFree(device_promoted);
        cudaFree(device_promotion);
        cudaFree(device_random7);
        cudaFree(device_output);
    };
    constexpr std::size_t u16_count = 65536u;
    const std::size_t matrix_count =
        static_cast<std::size_t>(context_count) * u16_count;
    if (cudaMalloc(&device_seeds, count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_contexts, u16_count) != cudaSuccess ||
        cudaMalloc(&device_normal, matrix_count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_promoted, matrix_count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_promotion, u16_count) != cudaSuccess ||
        cudaMalloc(&device_random7, u16_count) != cudaSuccess ||
        cudaMalloc(&device_output, count * sizeof(std::uint32_t)) != cudaSuccess) {
        cleanup();
        return false;
    }
    if (cudaMemcpy(device_seeds, seeds, count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_contexts, context_by_first_u16, u16_count, cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_normal, normal_lookups, matrix_count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_promoted, promoted_lookups, matrix_count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_promotion, promotion_success_lookup, u16_count, cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_random7, random7_lookup, u16_count, cudaMemcpyHostToDevice) != cudaSuccess) {
        cleanup();
        return false;
    }
    constexpr std::uint32_t threads = 256u;
    const std::uint32_t blocks = static_cast<std::uint32_t>(
        (count + threads - 1u) / threads);
    generate_ng3_r4_primary_ids_multi_kernel<<<blocks, threads>>>(
        device_seeds,
        count,
        device_contexts,
        device_normal,
        device_promoted,
        device_promotion,
        device_random7,
        device_output);
    const bool success =
        cudaGetLastError() == cudaSuccess &&
        cudaDeviceSynchronize() == cudaSuccess &&
        cudaMemcpy(
            output_effect_ids,
            device_output,
            count * sizeof(std::uint32_t),
            cudaMemcpyDeviceToHost) == cudaSuccess;
    cleanup();
    return success;
}

std::uint64_t collect_r4_primary_pivot_seeds_on_cuda(
    const std::uint16_t* values,
    std::uint32_t value_count,
    std::uint64_t start_index,
    std::uint64_t stop_index,
    std::uint16_t low16_stride,
    const std::uint32_t* allowed_effect_ids,
    std::uint32_t allowed_effect_count,
    const std::uint8_t* context_by_first_u16,
    std::uint32_t context_count,
    const std::uint32_t* normal_lookups,
    const std::uint32_t* promoted_lookups,
    const std::uint8_t* promotion_success_lookup,
    const std::uint8_t* random7_lookup,
    std::uint32_t* output_seeds,
    std::uint64_t* output_trials,
    std::uint64_t output_capacity) {
    const std::uint64_t item_count = stop_index - start_index;
    std::uint16_t* device_values = nullptr;
    std::uint32_t* device_allowed = nullptr;
    std::uint8_t* device_contexts = nullptr;
    std::uint32_t* device_normal = nullptr;
    std::uint32_t* device_promoted = nullptr;
    std::uint8_t* device_promotion = nullptr;
    std::uint8_t* device_random7 = nullptr;
    std::uint32_t* device_seeds = nullptr;
    std::uint64_t* device_trials = nullptr;
    unsigned long long* device_count = nullptr;
    auto cleanup = [&]() {
        cudaFree(device_values);
        cudaFree(device_allowed);
        cudaFree(device_contexts);
        cudaFree(device_normal);
        cudaFree(device_promoted);
        cudaFree(device_promotion);
        cudaFree(device_random7);
        cudaFree(device_seeds);
        cudaFree(device_trials);
        cudaFree(device_count);
    };
    constexpr std::size_t u16_count = 65536u;
    const std::size_t matrix_count =
        static_cast<std::size_t>(context_count) * u16_count;
    if (cudaMalloc(&device_values, value_count * sizeof(std::uint16_t)) != cudaSuccess ||
        cudaMalloc(&device_allowed, allowed_effect_count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_contexts, u16_count) != cudaSuccess ||
        cudaMalloc(&device_normal, matrix_count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_promoted, matrix_count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_promotion, u16_count) != cudaSuccess ||
        cudaMalloc(&device_random7, u16_count) != cudaSuccess ||
        cudaMalloc(&device_seeds, output_capacity * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_trials, output_capacity * sizeof(std::uint64_t)) != cudaSuccess ||
        cudaMalloc(&device_count, sizeof(unsigned long long)) != cudaSuccess) {
        cleanup();
        return UINT64_MAX;
    }
    if (cudaMemcpy(device_values, values, value_count * sizeof(std::uint16_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_allowed, allowed_effect_ids, allowed_effect_count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_contexts, context_by_first_u16, u16_count, cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_normal, normal_lookups, matrix_count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_promoted, promoted_lookups, matrix_count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_promotion, promotion_success_lookup, u16_count, cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_random7, random7_lookup, u16_count, cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemset(device_count, 0, sizeof(unsigned long long)) != cudaSuccess) {
        cleanup();
        return UINT64_MAX;
    }
    constexpr std::uint32_t threads = 256u;
    const std::uint32_t blocks = static_cast<std::uint32_t>(
        (item_count + threads - 1u) / threads);
    collect_ng3_r4_primary_pivot_seeds_kernel<<<blocks, threads>>>(
        device_values,
        value_count,
        start_index,
        item_count,
        low16_stride,
        device_allowed,
        allowed_effect_count,
        device_contexts,
        device_normal,
        device_promoted,
        device_promotion,
        device_random7,
        device_seeds,
        device_trials,
        device_count,
        output_capacity);
    unsigned long long count = 0u;
    if (cudaGetLastError() != cudaSuccess ||
        cudaDeviceSynchronize() != cudaSuccess ||
        cudaMemcpy(&count, device_count, sizeof(count), cudaMemcpyDeviceToHost) != cudaSuccess ||
        count > output_capacity ||
        cudaMemcpy(output_seeds, device_seeds, count * sizeof(std::uint32_t), cudaMemcpyDeviceToHost) != cudaSuccess ||
        cudaMemcpy(output_trials, device_trials, count * sizeof(std::uint64_t), cudaMemcpyDeviceToHost) != cudaSuccess) {
        cleanup();
        return UINT64_MAX;
    }
    cleanup();
    return static_cast<std::uint64_t>(count);
}

bool generate_terrain_rows_on_cuda(
    const std::uint32_t* seeds,
    std::uint64_t count,
    int mode_threshold,
    const std::uint32_t* filtered_rows,
    std::uint32_t filtered_count,
    std::uint32_t terrain_row_count,
    std::uint32_t* output_rows) {
    std::uint32_t* device_seeds = nullptr;
    std::uint32_t* device_filtered_rows = nullptr;
    std::uint32_t* device_output = nullptr;
    auto cleanup = [&]() {
        cudaFree(device_seeds);
        cudaFree(device_filtered_rows);
        cudaFree(device_output);
    };
    if (cudaMalloc(&device_seeds, count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(
            &device_filtered_rows,
            filtered_count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_output, count * sizeof(std::uint32_t)) != cudaSuccess) {
        cleanup();
        return false;
    }
    if (cudaMemcpy(
            device_seeds,
            seeds,
            count * sizeof(std::uint32_t),
            cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(
            device_filtered_rows,
            filtered_rows,
            filtered_count * sizeof(std::uint32_t),
            cudaMemcpyHostToDevice) != cudaSuccess) {
        cleanup();
        return false;
    }
    constexpr std::uint32_t threads = 256u;
    const std::uint32_t blocks = static_cast<std::uint32_t>(
        (count + threads - 1u) / threads);
    generate_terrain_row_indices_kernel<<<blocks, threads>>>(
        device_seeds,
        count,
        mode_threshold,
        device_filtered_rows,
        filtered_count,
        terrain_row_count,
        device_output);
    const bool success =
        cudaGetLastError() == cudaSuccess &&
        cudaDeviceSynchronize() == cudaSuccess &&
        cudaMemcpy(
            output_rows,
            device_output,
            count * sizeof(std::uint32_t),
            cudaMemcpyDeviceToHost) == cudaSuccess;
    cleanup();
    return success;
}

bool match_enemy_constraints_on_cuda(
    const std::uint32_t* seeds,
    const std::uint32_t* terrain_rows,
    std::uint64_t count,
    std::uint8_t playthrough,
    int mode_threshold,
    const int* descriptor_thresholds,
    int selector_threshold,
    int role_five_threshold,
    std::uint8_t selector_value,
    const EnemyCandidateInput* enemy_rows,
    std::uint32_t enemy_row_count,
    const EnemyTerrainInput* terrains,
    std::uint32_t terrain_count,
    const EnemyContextInput* contexts,
    std::uint32_t context_count,
    const std::uint32_t* criterion_keys,
    std::uint32_t criterion_key_count,
    const std::uint16_t* group_offsets,
    std::uint32_t group_count,
    std::uint32_t* output_masks) {
    std::uint32_t* device_seeds = nullptr;
    std::uint32_t* device_terrain_rows = nullptr;
    int* device_descriptor_thresholds = nullptr;
    EnemyCandidateInput* device_enemy_rows = nullptr;
    EnemyTerrainInput* device_terrains = nullptr;
    EnemyContextInput* device_contexts = nullptr;
    std::uint32_t* device_criterion_keys = nullptr;
    std::uint16_t* device_group_offsets = nullptr;
    std::uint32_t* device_output = nullptr;
    auto cleanup = [&]() {
        cudaFree(device_seeds);
        cudaFree(device_terrain_rows);
        cudaFree(device_descriptor_thresholds);
        cudaFree(device_enemy_rows);
        cudaFree(device_terrains);
        cudaFree(device_contexts);
        cudaFree(device_criterion_keys);
        cudaFree(device_group_offsets);
        cudaFree(device_output);
    };
    if (cudaMalloc(&device_seeds, count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_terrain_rows, count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_descriptor_thresholds, 3u * sizeof(int)) != cudaSuccess ||
        cudaMalloc(
            &device_enemy_rows,
            enemy_row_count * sizeof(EnemyCandidateInput)) != cudaSuccess ||
        cudaMalloc(
            &device_terrains,
            terrain_count * sizeof(EnemyTerrainInput)) != cudaSuccess ||
        cudaMalloc(
            &device_contexts,
            context_count * sizeof(EnemyContextInput)) != cudaSuccess ||
        cudaMalloc(
            &device_criterion_keys,
            criterion_key_count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(
            &device_group_offsets,
            (group_count + 1u) * sizeof(std::uint16_t)) != cudaSuccess ||
        cudaMalloc(&device_output, count * sizeof(std::uint32_t)) != cudaSuccess) {
        cleanup();
        return false;
    }
    if (cudaMemcpy(
            device_seeds,
            seeds,
            count * sizeof(std::uint32_t),
            cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(
            device_terrain_rows,
            terrain_rows,
            count * sizeof(std::uint32_t),
            cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(
            device_descriptor_thresholds,
            descriptor_thresholds,
            3u * sizeof(int),
            cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(
            device_enemy_rows,
            enemy_rows,
            enemy_row_count * sizeof(EnemyCandidateInput),
            cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(
            device_terrains,
            terrains,
            terrain_count * sizeof(EnemyTerrainInput),
            cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(
            device_contexts,
            contexts,
            context_count * sizeof(EnemyContextInput),
            cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(
            device_criterion_keys,
            criterion_keys,
            criterion_key_count * sizeof(std::uint32_t),
            cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(
            device_group_offsets,
            group_offsets,
            (group_count + 1u) * sizeof(std::uint16_t),
            cudaMemcpyHostToDevice) != cudaSuccess) {
        cleanup();
        return false;
    }
    constexpr std::uint32_t threads = 128u;
    const std::uint32_t blocks = static_cast<std::uint32_t>(
        (count + threads - 1u) / threads);
    match_enemy_constraints_kernel<<<blocks, threads>>>(
        device_seeds,
        device_terrain_rows,
        count,
        playthrough,
        mode_threshold,
        device_descriptor_thresholds,
        selector_threshold,
        role_five_threshold,
        selector_value,
        device_enemy_rows,
        enemy_row_count,
        device_terrains,
        terrain_count,
        device_contexts,
        context_count,
        device_criterion_keys,
        device_group_offsets,
        group_count,
        device_output);
    const bool success =
        cudaGetLastError() == cudaSuccess &&
        cudaDeviceSynchronize() == cudaSuccess &&
        cudaMemcpy(
            output_masks,
            device_output,
            count * sizeof(std::uint32_t),
            cudaMemcpyDeviceToHost) == cudaSuccess;
    cleanup();
    return success;
}

bool match_special_rule_constraints_on_cuda(
    const std::uint32_t* seeds,
    const std::uint32_t* scratch_masks,
    std::uint64_t count,
    const SpecialRuleInput* rows,
    std::uint32_t row_count,
    const std::uint16_t* criterion_keys,
    std::uint32_t criterion_key_count,
    const std::uint16_t* group_offsets,
    std::uint32_t group_count,
    std::uint32_t* output_masks) {
    std::uint32_t* device_seeds = nullptr;
    std::uint32_t* device_scratch_masks = nullptr;
    SpecialRuleInput* device_rows = nullptr;
    std::uint16_t* device_criterion_keys = nullptr;
    std::uint16_t* device_group_offsets = nullptr;
    std::uint32_t* device_output = nullptr;
    auto cleanup = [&]() {
        cudaFree(device_seeds);
        cudaFree(device_scratch_masks);
        cudaFree(device_rows);
        cudaFree(device_criterion_keys);
        cudaFree(device_group_offsets);
        cudaFree(device_output);
    };
    if (cudaMalloc(&device_seeds, count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(
            &device_scratch_masks,
            count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(
            &device_rows,
            row_count * sizeof(SpecialRuleInput)) != cudaSuccess ||
        cudaMalloc(
            &device_criterion_keys,
            criterion_key_count * sizeof(std::uint16_t)) != cudaSuccess ||
        cudaMalloc(
            &device_group_offsets,
            (group_count + 1u) * sizeof(std::uint16_t)) != cudaSuccess ||
        cudaMalloc(&device_output, count * sizeof(std::uint32_t)) != cudaSuccess) {
        cleanup();
        return false;
    }
    if (cudaMemcpy(
            device_seeds,
            seeds,
            count * sizeof(std::uint32_t),
            cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(
            device_scratch_masks,
            scratch_masks,
            count * sizeof(std::uint32_t),
            cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(
            device_rows,
            rows,
            row_count * sizeof(SpecialRuleInput),
            cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(
            device_criterion_keys,
            criterion_keys,
            criterion_key_count * sizeof(std::uint16_t),
            cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(
            device_group_offsets,
            group_offsets,
            (group_count + 1u) * sizeof(std::uint16_t),
            cudaMemcpyHostToDevice) != cudaSuccess) {
        cleanup();
        return false;
    }
    constexpr std::uint32_t threads = 128u;
    const std::uint32_t blocks = static_cast<std::uint32_t>(
        (count + threads - 1u) / threads);
    match_special_rule_constraints_kernel<<<blocks, threads>>>(
        device_seeds,
        device_scratch_masks,
        count,
        device_rows,
        row_count,
        device_criterion_keys,
        device_group_offsets,
        group_count,
        device_output);
    const bool success =
        cudaGetLastError() == cudaSuccess &&
        cudaDeviceSynchronize() == cudaSuccess &&
        cudaMemcpy(
            output_masks,
            device_output,
            count * sizeof(std::uint32_t),
            cudaMemcpyDeviceToHost) == cudaSuccess;
    cleanup();
    return success;
}

struct AuxiliaryMatcherView {
    std::uint8_t playthrough;
    int mode_threshold;
    const std::uint32_t* filtered_terrain_rows;
    std::uint32_t filtered_terrain_count;
    std::uint32_t terrain_row_count;
    const std::uint8_t* allowed_terrain_rows;
    const int* descriptor_thresholds;
    int selector_threshold;
    int role_five_threshold;
    std::uint8_t selector_value;
    const EnemyCandidateInput* enemy_rows;
    std::uint32_t enemy_row_count;
    const EnemyTerrainInput* terrains;
    std::uint32_t terrain_count;
    const EnemyContextInput* contexts;
    std::uint32_t context_count;
    const std::uint32_t* enemy_criterion_keys;
    const std::uint16_t* enemy_group_offsets;
    std::uint32_t enemy_group_count;
    std::uint32_t scratch_group_count;
    const SpecialRuleInput* rule_rows;
    std::uint32_t rule_row_count;
    const std::uint16_t* rule_criterion_keys;
    const std::uint16_t* rule_group_offsets;
    std::uint32_t rule_group_count;
};

struct AuxiliaryEvaluation {
    std::uint32_t terrain_row;
    bool terrain_matches;
    std::uint32_t enemy_mask;
    std::uint32_t rule_mask;
};

__host__ __device__ std::uint32_t target_group_mask(std::uint32_t count) {
    return count == 32u ? 0xFFFFFFFFu : (count == 0u ? 0u : ((1u << count) - 1u));
}

__host__ __device__ AuxiliaryEvaluation evaluate_auxiliary_constraints(
    std::uint32_t seed,
    const AuxiliaryMatcherView& view) {
    AuxiliaryEvaluation result{};
    result.terrain_row = generate_terrain_row_index(
        seed,
        view.mode_threshold,
        view.filtered_terrain_rows,
        view.filtered_terrain_count,
        view.terrain_row_count);
    result.terrain_matches =
        result.terrain_row < view.terrain_row_count &&
        view.allowed_terrain_rows[result.terrain_row] != 0u;
    if (!result.terrain_matches) {
        return result;
    }

    const std::uint32_t combined_enemy_group_count =
        view.enemy_group_count + view.scratch_group_count;
    std::uint32_t combined_enemy_mask = 0u;
    if (combined_enemy_group_count != 0u) {
        native_enemy_matcher::match_enemy_constraints_for_seed(
            seed,
            result.terrain_row,
            view.playthrough,
            view.mode_threshold,
            view.descriptor_thresholds,
            view.selector_threshold,
            view.role_five_threshold,
            view.selector_value,
            view.enemy_rows,
            view.enemy_row_count,
            view.terrains,
            view.terrain_count,
            view.contexts,
            view.context_count,
            view.enemy_criterion_keys,
            view.enemy_group_offsets,
            combined_enemy_group_count,
            &combined_enemy_mask);
    }
    result.enemy_mask = combined_enemy_mask & target_group_mask(view.enemy_group_count);
    if (result.enemy_mask != target_group_mask(view.enemy_group_count)) {
        return result;
    }
    if (view.rule_group_count != 0u) {
        const std::uint32_t scratch_mask =
            (combined_enemy_mask >> view.enemy_group_count) &
            target_group_mask(view.scratch_group_count);
        result.rule_mask = match_special_rules_for_seed(
            seed,
            scratch_mask,
            view.rule_rows,
            view.rule_row_count,
            view.rule_criterion_keys,
            view.rule_group_offsets,
            view.rule_group_count);
    }
    return result;
}

__global__ void collect_auxiliary_natural_seeds_kernel(
    const std::uint16_t* values,
    std::uint32_t value_count,
    std::uint64_t start_index,
    std::uint64_t item_count,
    std::uint16_t low16_stride,
    std::uint32_t draw_index,
    std::uint32_t* natural_seeds,
    std::uint64_t* natural_trials,
    unsigned long long* natural_count,
    std::uint64_t natural_capacity) {
    const std::uint64_t item_index =
        static_cast<std::uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (item_index >= item_count) {
        return;
    }
    const std::uint64_t flat_index = start_index + item_index;
    const std::uint64_t low_index = flat_index / value_count;
    const std::uint32_t bucket_index =
        static_cast<std::uint32_t>(flat_index % value_count);
    const std::uint16_t low16 = static_cast<std::uint16_t>(
        static_cast<std::uint32_t>(low_index) * low16_stride);
    const std::uint32_t rotation =
        static_cast<std::uint32_t>(low_index % value_count);
    const std::uint16_t high16 = values[(rotation + bucket_index) % value_count];
    std::uint32_t seed = (static_cast<std::uint32_t>(high16) << 16u) | low16;
    for (std::uint32_t draw = 0u; draw < draw_index; ++draw) {
        seed = kLcgInverse * (seed - 1u);
    }
    if (!is_natural_seed(seed)) {
        return;
    }
    const unsigned long long output_index = atomicAdd(natural_count, 1ull);
    if (output_index < natural_capacity) {
        natural_seeds[output_index] = seed;
        natural_trials[output_index] = flat_index + 1u;
    }
}

__global__ void collect_auxiliary_matches_kernel(
    const std::uint32_t* natural_seeds,
    const std::uint64_t* natural_trials,
    std::uint64_t natural_count,
    AuxiliaryMatcherView view,
    bool has_terrain_constraint,
    std::uint32_t* output_seeds,
    std::uint64_t* output_trials,
    unsigned long long* output_count,
    std::uint64_t output_capacity,
    unsigned long long* stage_counts) {
    const std::uint64_t index =
        static_cast<std::uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index >= natural_count) {
        return;
    }
    const AuxiliaryEvaluation result = evaluate_auxiliary_constraints(
        natural_seeds[index], view);
    std::uint32_t stage_index = 1u;
    if (has_terrain_constraint) {
        if (!result.terrain_matches) {
            return;
        }
        atomicAdd(stage_counts + stage_index, 1ull);
        ++stage_index;
    }
    for (std::uint32_t group = 0u; group < view.enemy_group_count; ++group) {
        const std::uint32_t prefix = target_group_mask(group + 1u);
        if ((result.enemy_mask & prefix) != prefix) {
            return;
        }
        atomicAdd(stage_counts + stage_index, 1ull);
        ++stage_index;
    }
    for (std::uint32_t group = 0u; group < view.rule_group_count; ++group) {
        const std::uint32_t prefix = target_group_mask(group + 1u);
        if ((result.rule_mask & prefix) != prefix) {
            return;
        }
        atomicAdd(stage_counts + stage_index, 1ull);
        ++stage_index;
    }
    if (!result.terrain_matches ||
        result.enemy_mask != target_group_mask(view.enemy_group_count) ||
        result.rule_mask != target_group_mask(view.rule_group_count)) {
        return;
    }
    const unsigned long long output_index = atomicAdd(output_count, 1ull);
    if (output_index < output_capacity) {
        output_seeds[output_index] = natural_seeds[index];
        output_trials[output_index] = natural_trials[index];
    }
}

bool collect_auxiliary_pivot_matches_on_cuda(
    const std::uint16_t* values,
    std::uint32_t value_count,
    std::uint64_t start_index,
    std::uint64_t stop_index,
    std::uint16_t low16_stride,
    std::uint32_t draw_index,
    const AuxiliaryMatcherView& host_view,
    bool has_terrain_constraint,
    std::uint32_t* output_seeds,
    std::uint64_t* output_trials,
    std::uint64_t output_capacity,
    std::uint64_t* output_stage_counts,
    std::uint32_t stage_count) {
    const std::uint64_t item_count = stop_index - start_index;
    std::uint16_t* device_values = nullptr;
    std::uint32_t* device_natural_seeds = nullptr;
    std::uint64_t* device_natural_trials = nullptr;
    std::uint32_t* device_output_seeds = nullptr;
    std::uint64_t* device_output_trials = nullptr;
    unsigned long long* device_natural_count = nullptr;
    unsigned long long* device_output_count = nullptr;
    unsigned long long* device_stage_counts = nullptr;
    std::uint32_t* device_filtered_rows = nullptr;
    std::uint8_t* device_allowed_terrain_rows = nullptr;
    int* device_descriptor_thresholds = nullptr;
    EnemyCandidateInput* device_enemy_rows = nullptr;
    EnemyTerrainInput* device_terrains = nullptr;
    EnemyContextInput* device_contexts = nullptr;
    std::uint32_t* device_enemy_keys = nullptr;
    std::uint16_t* device_enemy_offsets = nullptr;
    SpecialRuleInput* device_rule_rows = nullptr;
    std::uint16_t* device_rule_keys = nullptr;
    std::uint16_t* device_rule_offsets = nullptr;
    auto cleanup = [&]() {
        cudaFree(device_values);
        cudaFree(device_natural_seeds);
        cudaFree(device_natural_trials);
        cudaFree(device_output_seeds);
        cudaFree(device_output_trials);
        cudaFree(device_natural_count);
        cudaFree(device_output_count);
        cudaFree(device_stage_counts);
        cudaFree(device_filtered_rows);
        cudaFree(device_allowed_terrain_rows);
        cudaFree(device_descriptor_thresholds);
        cudaFree(device_enemy_rows);
        cudaFree(device_terrains);
        cudaFree(device_contexts);
        cudaFree(device_enemy_keys);
        cudaFree(device_enemy_offsets);
        cudaFree(device_rule_rows);
        cudaFree(device_rule_keys);
        cudaFree(device_rule_offsets);
    };
    const std::uint32_t combined_enemy_groups =
        host_view.enemy_group_count + host_view.scratch_group_count;
    const std::uint32_t enemy_key_count = combined_enemy_groups == 0u
        ? 0u
        : host_view.enemy_group_offsets[combined_enemy_groups];
    const std::uint32_t rule_key_count = host_view.rule_group_count == 0u
        ? 0u
        : host_view.rule_group_offsets[host_view.rule_group_count];
    if (cudaMalloc(&device_values, value_count * sizeof(std::uint16_t)) != cudaSuccess ||
        cudaMalloc(&device_natural_seeds, item_count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_natural_trials, item_count * sizeof(std::uint64_t)) != cudaSuccess ||
        cudaMalloc(&device_output_seeds, output_capacity * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_output_trials, output_capacity * sizeof(std::uint64_t)) != cudaSuccess ||
        cudaMalloc(&device_natural_count, sizeof(unsigned long long)) != cudaSuccess ||
        cudaMalloc(&device_output_count, sizeof(unsigned long long)) != cudaSuccess ||
        cudaMalloc(&device_stage_counts, stage_count * sizeof(unsigned long long)) != cudaSuccess ||
        cudaMalloc(&device_filtered_rows, host_view.filtered_terrain_count * sizeof(std::uint32_t)) != cudaSuccess ||
        cudaMalloc(&device_allowed_terrain_rows, host_view.terrain_row_count * sizeof(std::uint8_t)) != cudaSuccess ||
        cudaMalloc(&device_descriptor_thresholds, 3u * sizeof(int)) != cudaSuccess ||
        cudaMalloc(&device_enemy_rows, host_view.enemy_row_count * sizeof(EnemyCandidateInput)) != cudaSuccess ||
        cudaMalloc(&device_terrains, host_view.terrain_count * sizeof(EnemyTerrainInput)) != cudaSuccess ||
        cudaMalloc(&device_contexts, host_view.context_count * sizeof(EnemyContextInput)) != cudaSuccess ||
        (enemy_key_count != 0u && cudaMalloc(&device_enemy_keys, enemy_key_count * sizeof(std::uint32_t)) != cudaSuccess) ||
        (combined_enemy_groups != 0u && cudaMalloc(&device_enemy_offsets, (combined_enemy_groups + 1u) * sizeof(std::uint16_t)) != cudaSuccess) ||
        (host_view.rule_group_count != 0u && cudaMalloc(&device_rule_rows, host_view.rule_row_count * sizeof(SpecialRuleInput)) != cudaSuccess) ||
        (rule_key_count != 0u && cudaMalloc(&device_rule_keys, rule_key_count * sizeof(std::uint16_t)) != cudaSuccess) ||
        (host_view.rule_group_count != 0u && cudaMalloc(&device_rule_offsets, (host_view.rule_group_count + 1u) * sizeof(std::uint16_t)) != cudaSuccess)) {
        cleanup();
        return false;
    }
    unsigned long long zero = 0u;
    if (cudaMemcpy(device_values, values, value_count * sizeof(std::uint16_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_filtered_rows, host_view.filtered_terrain_rows, host_view.filtered_terrain_count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_allowed_terrain_rows, host_view.allowed_terrain_rows, host_view.terrain_row_count * sizeof(std::uint8_t), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_descriptor_thresholds, host_view.descriptor_thresholds, 3u * sizeof(int), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_enemy_rows, host_view.enemy_rows, host_view.enemy_row_count * sizeof(EnemyCandidateInput), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_terrains, host_view.terrains, host_view.terrain_count * sizeof(EnemyTerrainInput), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_contexts, host_view.contexts, host_view.context_count * sizeof(EnemyContextInput), cudaMemcpyHostToDevice) != cudaSuccess ||
        (enemy_key_count != 0u && cudaMemcpy(device_enemy_keys, host_view.enemy_criterion_keys, enemy_key_count * sizeof(std::uint32_t), cudaMemcpyHostToDevice) != cudaSuccess) ||
        (combined_enemy_groups != 0u && cudaMemcpy(device_enemy_offsets, host_view.enemy_group_offsets, (combined_enemy_groups + 1u) * sizeof(std::uint16_t), cudaMemcpyHostToDevice) != cudaSuccess) ||
        (host_view.rule_group_count != 0u && cudaMemcpy(device_rule_rows, host_view.rule_rows, host_view.rule_row_count * sizeof(SpecialRuleInput), cudaMemcpyHostToDevice) != cudaSuccess) ||
        (rule_key_count != 0u && cudaMemcpy(device_rule_keys, host_view.rule_criterion_keys, rule_key_count * sizeof(std::uint16_t), cudaMemcpyHostToDevice) != cudaSuccess) ||
        (host_view.rule_group_count != 0u && cudaMemcpy(device_rule_offsets, host_view.rule_group_offsets, (host_view.rule_group_count + 1u) * sizeof(std::uint16_t), cudaMemcpyHostToDevice) != cudaSuccess) ||
        cudaMemcpy(device_natural_count, &zero, sizeof(zero), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemcpy(device_output_count, &zero, sizeof(zero), cudaMemcpyHostToDevice) != cudaSuccess ||
        cudaMemset(device_stage_counts, 0, stage_count * sizeof(unsigned long long)) != cudaSuccess) {
        cleanup();
        return false;
    }
    constexpr std::uint32_t threads = 128u;
    const std::uint32_t raw_blocks = static_cast<std::uint32_t>(
        (item_count + threads - 1u) / threads);
    collect_auxiliary_natural_seeds_kernel<<<raw_blocks, threads>>>(
        device_values,
        value_count,
        start_index,
        item_count,
        low16_stride,
        draw_index,
        device_natural_seeds,
        device_natural_trials,
        device_natural_count,
        item_count);
    unsigned long long natural_count = 0u;
    if (cudaGetLastError() != cudaSuccess ||
        cudaMemcpy(&natural_count, device_natural_count, sizeof(natural_count), cudaMemcpyDeviceToHost) != cudaSuccess ||
        natural_count > item_count) {
        cleanup();
        return false;
    }
    AuxiliaryMatcherView device_view = host_view;
    device_view.filtered_terrain_rows = device_filtered_rows;
    device_view.allowed_terrain_rows = device_allowed_terrain_rows;
    device_view.descriptor_thresholds = device_descriptor_thresholds;
    device_view.enemy_rows = device_enemy_rows;
    device_view.terrains = device_terrains;
    device_view.contexts = device_contexts;
    device_view.enemy_criterion_keys = device_enemy_keys;
    device_view.enemy_group_offsets = device_enemy_offsets;
    device_view.rule_rows = device_rule_rows;
    device_view.rule_criterion_keys = device_rule_keys;
    device_view.rule_group_offsets = device_rule_offsets;
    if (natural_count != 0u) {
        const std::uint32_t match_blocks = static_cast<std::uint32_t>(
            (natural_count + threads - 1u) / threads);
        collect_auxiliary_matches_kernel<<<match_blocks, threads>>>(
            device_natural_seeds,
            device_natural_trials,
            natural_count,
            device_view,
            has_terrain_constraint,
            device_output_seeds,
            device_output_trials,
            device_output_count,
            output_capacity,
            device_stage_counts);
    }
    unsigned long long match_count = 0u;
    if (cudaGetLastError() != cudaSuccess || cudaDeviceSynchronize() != cudaSuccess ||
        cudaMemcpy(&match_count, device_output_count, sizeof(match_count), cudaMemcpyDeviceToHost) != cudaSuccess ||
        match_count > output_capacity ||
        cudaMemcpy(output_stage_counts, device_stage_counts, stage_count * sizeof(unsigned long long), cudaMemcpyDeviceToHost) != cudaSuccess ||
        (match_count != 0u && cudaMemcpy(output_seeds, device_output_seeds, match_count * sizeof(std::uint32_t), cudaMemcpyDeviceToHost) != cudaSuccess) ||
        (match_count != 0u && cudaMemcpy(output_trials, device_output_trials, match_count * sizeof(std::uint64_t), cudaMemcpyDeviceToHost) != cudaSuccess)) {
        cleanup();
        return false;
    }
    output_stage_counts[0] = natural_count;
    cleanup();
    return true;
}

}  // namespace

extern "C" __declspec(dllexport) int cuda_seed_acceleration_available() {
    int device_count = 0;
    return cudaGetDeviceCount(&device_count) == cudaSuccess && device_count > 0 ? 1 : 0;
}

extern "C" __declspec(dllexport) int seed_accelerator_last_backend() {
    return g_last_backend;
}

extern "C" __declspec(dllexport) int build_weighted_effect_lookup(
    const std::uint32_t* effect_ids,
    const std::uint32_t* weights,
    std::uint32_t entry_count,
    std::uint32_t* output_effect_ids) {
    if (effect_ids == nullptr || weights == nullptr || entry_count == 0u ||
        entry_count > 4096u || output_effect_ids == nullptr) {
        return -1;
    }
    std::uint32_t total = 0u;
    for (std::uint32_t index = 0u; index < entry_count; ++index) {
        total += weights[index];
    }
    const std::uint32_t upper_count = total + 1u;
    if (upper_count == 0u) {
        return -1;
    }
    for (std::uint32_t value = 0u; value < 65536u; ++value) {
        volatile float random_float =
            static_cast<float>(value) * (1.0f / 65536.0f);
        volatile float count_float = static_cast<float>(upper_count);
        volatile float scaled = random_float * count_float;
        std::uint32_t ticket = static_cast<std::uint32_t>(scaled);
        if (ticket >= upper_count) {
            ticket = upper_count - 1u;
        }
        if (ticket > total) {
            ticket = total;
        }
        bool selected = false;
        for (std::uint32_t index = 0u; index < entry_count; ++index) {
            if (ticket <= weights[index]) {
                output_effect_ids[value] = effect_ids[index];
                selected = true;
                break;
            }
            ticket -= weights[index];
        }
        if (!selected) {
            return -1;
        }
    }
    return 0;
}

extern "C" __declspec(dllexport) int generate_ng3_primary_effect_ids(
    const std::uint32_t* seeds,
    std::uint64_t count,
    const std::uint32_t* normal_lookup,
    const std::uint32_t* promoted_lookup,
    const std::uint8_t* promotion_success_lookup,
    const std::uint8_t* random7_lookup,
    std::uint32_t* output_effect_ids) {
    if (seeds == nullptr || count == 0u || count > 1000000u ||
        normal_lookup == nullptr || promoted_lookup == nullptr ||
        promotion_success_lookup == nullptr || random7_lookup == nullptr ||
        output_effect_ids == nullptr) {
        return -1;
    }
    int device_count = 0;
    if (cudaGetDeviceCount(&device_count) == cudaSuccess && device_count > 0 &&
        generate_primary_ids_on_cuda(
            seeds,
            count,
            normal_lookup,
            promoted_lookup,
            promotion_success_lookup,
            random7_lookup,
            output_effect_ids)) {
        g_last_backend = 1;
        return 1;
    }
    for (std::uint64_t index = 0u; index < count; ++index) {
        std::uint32_t state = lcg_step(seeds[index]);
        state = lcg_step(state);
        const bool promoted = promotion_success_lookup[state >> 16u] != 0u;
        bool primary_promoted = false;
        if (promoted) {
            std::uint8_t order[7] = {0, 1, 2, 3, 4, 5, 6};
            for (std::uint32_t position = 0u; position < 7u; ++position) {
                state = lcg_step(state);
                const std::uint8_t swap_index = random7_lookup[state >> 16u];
                const std::uint8_t temporary = order[position];
                order[position] = order[swap_index];
                order[swap_index] = temporary;
            }
            for (std::uint8_t selected : order) {
                if (selected > 0u && selected < 6u) {
                    primary_promoted = selected == 1u;
                    break;
                }
            }
        }
        state = lcg_step(state);
        const std::uint16_t lottery_u16 = static_cast<std::uint16_t>(state >> 16u);
        output_effect_ids[index] = primary_promoted
            ? promoted_lookup[lottery_u16]
            : normal_lookup[lottery_u16];
    }
    g_last_backend = 0;
    return 0;
}

extern "C" __declspec(dllexport) int generate_ng3_primary_effect_ids_context(
    const std::uint32_t* seeds,
    std::uint64_t count,
    const std::uint32_t* normal_lookup,
    const std::uint32_t* promoted_lookup,
    const std::uint8_t* promotion_success_lookup,
    const std::uint8_t* random7_lookup,
    std::uint32_t pre_promotion_draws,
    std::uint8_t slot_limit,
    std::uint8_t excluded_slot_mask,
    std::uint8_t primary_source_index,
    std::uint32_t* output_effect_ids) {
    if (seeds == nullptr || count == 0u || count > 1000000u ||
        normal_lookup == nullptr || promoted_lookup == nullptr ||
        promotion_success_lookup == nullptr || random7_lookup == nullptr ||
        slot_limit == 0u || slot_limit > 7u || primary_source_index >= slot_limit ||
        (excluded_slot_mask & (1u << primary_source_index)) != 0u ||
        output_effect_ids == nullptr) {
        return -1;
    }
    int device_count = 0;
    if (cudaGetDeviceCount(&device_count) == cudaSuccess && device_count > 0 &&
        generate_primary_ids_context_on_cuda(
            seeds,
            count,
            normal_lookup,
            promoted_lookup,
            promotion_success_lookup,
            random7_lookup,
            pre_promotion_draws,
            slot_limit,
            excluded_slot_mask,
            primary_source_index,
            output_effect_ids)) {
        g_last_backend = 1;
        return 1;
    }
    for (std::uint64_t index = 0u; index < count; ++index) {
        std::uint32_t state = seeds[index];
        for (std::uint32_t draw = 0u; draw < pre_promotion_draws; ++draw) {
            state = lcg_step(state);
        }
        state = lcg_step(state);
        const bool promoted = promotion_success_lookup[state >> 16u] != 0u;
        bool primary_promoted = false;
        if (promoted) {
            std::uint8_t order[7] = {0, 1, 2, 3, 4, 5, 6};
            for (std::uint32_t position = 0u; position < 7u; ++position) {
                state = lcg_step(state);
                const std::uint8_t swap_index = random7_lookup[state >> 16u];
                const std::uint8_t temporary = order[position];
                order[position] = order[swap_index];
                order[swap_index] = temporary;
            }
            for (std::uint8_t selected : order) {
                if (selected >= slot_limit ||
                    (excluded_slot_mask & (1u << selected)) != 0u) {
                    continue;
                }
                primary_promoted = selected == primary_source_index;
                break;
            }
        }
        state = lcg_step(state);
        const std::uint16_t lottery_u16 = static_cast<std::uint16_t>(state >> 16u);
        output_effect_ids[index] = primary_promoted
            ? promoted_lookup[lottery_u16]
            : normal_lookup[lottery_u16];
    }
    g_last_backend = 0;
    return 0;
}

extern "C" __declspec(dllexport) int generate_ng3_r4_primary_effect_ids_multi(
    const std::uint32_t* seeds,
    std::uint64_t count,
    const std::uint8_t* context_by_first_u16,
    std::uint32_t context_count,
    const std::uint32_t* normal_lookups,
    const std::uint32_t* promoted_lookups,
    const std::uint8_t* promotion_success_lookup,
    const std::uint8_t* random7_lookup,
    std::uint32_t* output_effect_ids) {
    if (seeds == nullptr || count == 0u || count > 1000000u ||
        context_by_first_u16 == nullptr || context_count == 0u ||
        context_count > 256u || normal_lookups == nullptr ||
        promoted_lookups == nullptr || promotion_success_lookup == nullptr ||
        random7_lookup == nullptr || output_effect_ids == nullptr) {
        return -1;
    }
    for (std::uint32_t value = 0u; value < 65536u; ++value) {
        if (context_by_first_u16[value] >= context_count) {
            return -1;
        }
    }
    int device_count = 0;
    if (cudaGetDeviceCount(&device_count) == cudaSuccess && device_count > 0 &&
        generate_r4_primary_ids_multi_on_cuda(
            seeds,
            count,
            context_by_first_u16,
            context_count,
            normal_lookups,
            promoted_lookups,
            promotion_success_lookup,
            random7_lookup,
            output_effect_ids)) {
        g_last_backend = 1;
        return 1;
    }
    for (std::uint64_t index = 0u; index < count; ++index) {
        std::uint32_t state = lcg_step(seeds[index]);
        const std::uint32_t context = context_by_first_u16[state >> 16u];
        state = lcg_step(state);
        const bool promoted = promotion_success_lookup[state >> 16u] != 0u;
        bool primary_promoted = false;
        if (promoted) {
            std::uint8_t order[7] = {0, 1, 2, 3, 4, 5, 6};
            for (std::uint32_t position = 0u; position < 7u; ++position) {
                state = lcg_step(state);
                const std::uint8_t swap_index = random7_lookup[state >> 16u];
                const std::uint8_t temporary = order[position];
                order[position] = order[swap_index];
                order[swap_index] = temporary;
            }
            for (std::uint8_t selected : order) {
                if (selected >= 5u || selected == 0u) {
                    continue;
                }
                primary_promoted = selected == 1u;
                break;
            }
        }
        state = lcg_step(state);
        const std::uint32_t lookup_index =
            context * 65536u + (state >> 16u);
        output_effect_ids[index] = primary_promoted
            ? promoted_lookups[lookup_index]
            : normal_lookups[lookup_index];
    }
    g_last_backend = 0;
    return 0;
}

extern "C" __declspec(dllexport) std::uint64_t collect_ng3_r4_primary_pivot_seeds(
    const std::uint16_t* values,
    std::uint32_t value_count,
    std::uint64_t start_index,
    std::uint64_t stop_index,
    std::uint16_t low16_stride,
    const std::uint32_t* allowed_effect_ids,
    std::uint32_t allowed_effect_count,
    const std::uint8_t* context_by_first_u16,
    std::uint32_t context_count,
    const std::uint32_t* normal_lookups,
    const std::uint32_t* promoted_lookups,
    const std::uint8_t* promotion_success_lookup,
    const std::uint8_t* random7_lookup,
    std::uint32_t* output_seeds,
    std::uint64_t* output_trials,
    std::uint64_t output_capacity) {
    if (values == nullptr || value_count == 0u ||
        start_index > stop_index || stop_index - start_index > 50000000u ||
        low16_stride == 0u || (low16_stride & 1u) == 0u ||
        allowed_effect_ids == nullptr || allowed_effect_count == 0u ||
        context_by_first_u16 == nullptr || context_count == 0u ||
        context_count > 256u || normal_lookups == nullptr ||
        promoted_lookups == nullptr || promotion_success_lookup == nullptr ||
        random7_lookup == nullptr || output_seeds == nullptr ||
        output_trials == nullptr || output_capacity == 0u) {
        return UINT64_MAX;
    }
    for (std::uint32_t value = 0u; value < 65536u; ++value) {
        if (context_by_first_u16[value] >= context_count) {
            return UINT64_MAX;
        }
    }
    int device_count = 0;
    if (cudaGetDeviceCount(&device_count) == cudaSuccess && device_count > 0) {
        const std::uint64_t count = collect_r4_primary_pivot_seeds_on_cuda(
            values,
            value_count,
            start_index,
            stop_index,
            low16_stride,
            allowed_effect_ids,
            allowed_effect_count,
            context_by_first_u16,
            context_count,
            normal_lookups,
            promoted_lookups,
            promotion_success_lookup,
            random7_lookup,
            output_seeds,
            output_trials,
            output_capacity);
        if (count != UINT64_MAX) {
            g_last_backend = 1;
            return count;
        }
    }

    std::uint64_t output_count = 0u;
    for (std::uint64_t flat_index = start_index; flat_index < stop_index; ++flat_index) {
        const std::uint64_t low_index = flat_index / value_count;
        const std::uint32_t bucket_index =
            static_cast<std::uint32_t>(flat_index % value_count);
        const std::uint16_t low16 = static_cast<std::uint16_t>(
            static_cast<std::uint32_t>(low_index) * low16_stride);
        const std::uint32_t rotation =
            static_cast<std::uint32_t>(low_index % value_count);
        const std::uint16_t high16 = values[(rotation + bucket_index) % value_count];
        const std::uint32_t pivot_state =
            (static_cast<std::uint32_t>(high16) << 16u) | low16;
        const std::uint32_t seed = kLcgInverse * (pivot_state - 1u);
        if (!is_natural_seed(seed)) {
            continue;
        }
        std::uint32_t state = lcg_step(seed);
        const std::uint32_t context = context_by_first_u16[state >> 16u];
        state = lcg_step(state);
        const bool promoted = promotion_success_lookup[state >> 16u] != 0u;
        bool primary_promoted = false;
        if (promoted) {
            std::uint8_t order[7] = {0, 1, 2, 3, 4, 5, 6};
            for (std::uint32_t position = 0u; position < 7u; ++position) {
                state = lcg_step(state);
                const std::uint8_t swap_index = random7_lookup[state >> 16u];
                const std::uint8_t temporary = order[position];
                order[position] = order[swap_index];
                order[swap_index] = temporary;
            }
            for (std::uint8_t selected : order) {
                if (selected >= 5u || selected == 0u) {
                    continue;
                }
                primary_promoted = selected == 1u;
                break;
            }
        }
        state = lcg_step(state);
        const std::uint32_t lookup_index = context * 65536u + (state >> 16u);
        const std::uint32_t effect_id = primary_promoted
            ? promoted_lookups[lookup_index]
            : normal_lookups[lookup_index];
        bool matches = false;
        for (std::uint32_t index = 0u; index < allowed_effect_count; ++index) {
            if (effect_id == allowed_effect_ids[index]) {
                matches = true;
                break;
            }
        }
        if (!matches) {
            continue;
        }
        if (output_count >= output_capacity) {
            return UINT64_MAX;
        }
        output_seeds[output_count] = seed;
        output_trials[output_count] = flat_index + 1u;
        ++output_count;
    }
    g_last_backend = 0;
    return output_count;
}

extern "C" __declspec(dllexport) int generate_terrain_row_indices(
    const std::uint32_t* seeds,
    std::uint64_t count,
    int mode_threshold,
    const std::uint32_t* filtered_rows,
    std::uint32_t filtered_count,
    std::uint32_t terrain_row_count,
    std::uint32_t* output_rows) {
    if (seeds == nullptr || count == 0u || count > 1000000u ||
        filtered_rows == nullptr || filtered_count == 0u ||
        terrain_row_count == 0u || output_rows == nullptr) {
        return -1;
    }
    for (std::uint32_t index = 0u; index < filtered_count; ++index) {
        if (filtered_rows[index] >= terrain_row_count) {
            return -1;
        }
    }
    int device_count = 0;
    if (cudaGetDeviceCount(&device_count) == cudaSuccess && device_count > 0 &&
        generate_terrain_rows_on_cuda(
            seeds,
            count,
            mode_threshold,
            filtered_rows,
            filtered_count,
            terrain_row_count,
            output_rows)) {
        g_last_backend = 1;
        return 1;
    }
    for (std::uint64_t index = 0u; index < count; ++index) {
        output_rows[index] = generate_terrain_row_index(
            seeds[index],
            mode_threshold,
            filtered_rows,
            filtered_count,
            terrain_row_count);
    }
    g_last_backend = 0;
    return 0;
}

extern "C" __declspec(dllexport) int match_enemy_constraints(
    const std::uint32_t* seeds,
    const std::uint32_t* terrain_rows,
    std::uint64_t count,
    std::uint8_t playthrough,
    int mode_threshold,
    const int* descriptor_thresholds,
    int selector_threshold,
    int role_five_threshold,
    std::uint8_t selector_value,
    const EnemyCandidateInput* enemy_rows,
    std::uint32_t enemy_row_count,
    const EnemyTerrainInput* terrains,
    std::uint32_t terrain_count,
    const EnemyContextInput* contexts,
    std::uint32_t context_count,
    const std::uint32_t* criterion_keys,
    std::uint32_t criterion_key_count,
    const std::uint16_t* group_offsets,
    std::uint32_t group_count,
    std::uint32_t* output_masks) {
    if (seeds == nullptr || terrain_rows == nullptr || count == 0u ||
        count > 1000000u || playthrough == 0u || playthrough > 5u ||
        descriptor_thresholds == nullptr || enemy_rows == nullptr ||
        enemy_row_count == 0u ||
        enemy_row_count > native_enemy_matcher::kMaximumEnemyRows ||
        terrains == nullptr || terrain_count == 0u || contexts == nullptr ||
        context_count == 0u || criterion_keys == nullptr ||
        criterion_key_count == 0u || group_offsets == nullptr ||
        group_count == 0u ||
        group_count > native_enemy_matcher::kMaximumCriteriaGroups ||
        group_offsets[0] != 0u ||
        group_offsets[group_count] != criterion_key_count ||
        output_masks == nullptr) {
        return -1;
    }
    for (std::uint32_t index = 0u; index < count; ++index) {
        if (terrain_rows[index] >= terrain_count) {
            return -1;
        }
    }
    for (std::uint32_t group = 0u; group < group_count; ++group) {
        if (group_offsets[group] >= group_offsets[group + 1u]) {
            return -1;
        }
    }
    int device_count = 0;
    if (cudaGetDeviceCount(&device_count) == cudaSuccess && device_count > 0 &&
        match_enemy_constraints_on_cuda(
            seeds,
            terrain_rows,
            count,
            playthrough,
            mode_threshold,
            descriptor_thresholds,
            selector_threshold,
            role_five_threshold,
            selector_value,
            enemy_rows,
            enemy_row_count,
            terrains,
            terrain_count,
            contexts,
            context_count,
            criterion_keys,
            criterion_key_count,
            group_offsets,
            group_count,
            output_masks)) {
        g_last_backend = 1;
        return 1;
    }
    for (std::uint64_t index = 0u; index < count; ++index) {
        std::uint32_t observed_mask = 0u;
        native_enemy_matcher::match_enemy_constraints_for_seed(
                seeds[index],
                terrain_rows[index],
                playthrough,
                mode_threshold,
                descriptor_thresholds,
                selector_threshold,
                role_five_threshold,
                selector_value,
                enemy_rows,
                enemy_row_count,
                terrains,
                terrain_count,
                contexts,
                context_count,
                criterion_keys,
                group_offsets,
                group_count,
                &observed_mask);
        output_masks[index] = observed_mask;
    }
    g_last_backend = 0;
    return 0;
}

extern "C" __declspec(dllexport) int match_special_rule_constraints(
    const std::uint32_t* seeds,
    const std::uint32_t* scratch_masks,
    std::uint64_t count,
    const SpecialRuleInput* rows,
    std::uint32_t row_count,
    const std::uint16_t* criterion_keys,
    std::uint32_t criterion_key_count,
    const std::uint16_t* group_offsets,
    std::uint32_t group_count,
    std::uint32_t* output_masks) {
    if (seeds == nullptr || scratch_masks == nullptr || count == 0u ||
        count > 1000000u || rows == nullptr || row_count == 0u ||
        row_count > 1024u || criterion_keys == nullptr ||
        criterion_key_count == 0u || group_offsets == nullptr ||
        group_count == 0u ||
        group_count > native_enemy_matcher::kMaximumCriteriaGroups ||
        group_offsets[0] != 0u ||
        group_offsets[group_count] != criterion_key_count ||
        output_masks == nullptr) {
        return -1;
    }
    for (std::uint32_t group = 0u; group < group_count; ++group) {
        if (group_offsets[group] >= group_offsets[group + 1u]) {
            return -1;
        }
    }
    int device_count = 0;
    if (cudaGetDeviceCount(&device_count) == cudaSuccess && device_count > 0 &&
        match_special_rule_constraints_on_cuda(
            seeds,
            scratch_masks,
            count,
            rows,
            row_count,
            criterion_keys,
            criterion_key_count,
            group_offsets,
            group_count,
            output_masks)) {
        g_last_backend = 1;
        return 1;
    }
    for (std::uint64_t index = 0u; index < count; ++index) {
        output_masks[index] = match_special_rules_for_seed(
            seeds[index],
            scratch_masks[index],
            rows,
            row_count,
            criterion_keys,
            group_offsets,
            group_count);
    }
    g_last_backend = 0;
    return 0;
}

extern "C" __declspec(dllexport) std::uint64_t collect_auxiliary_pivot_matches(
    const std::uint16_t* values,
    std::uint32_t value_count,
    std::uint64_t start_index,
    std::uint64_t stop_index,
    std::uint16_t low16_stride,
    std::uint32_t draw_index,
    std::uint8_t playthrough,
    int mode_threshold,
    const std::uint32_t* filtered_terrain_rows,
    std::uint32_t filtered_terrain_count,
    std::uint32_t terrain_row_count,
    const std::uint8_t* allowed_terrain_rows,
    std::uint32_t has_terrain_constraint,
    const int* descriptor_thresholds,
    int selector_threshold,
    int role_five_threshold,
    std::uint8_t selector_value,
    const EnemyCandidateInput* enemy_rows,
    std::uint32_t enemy_row_count,
    const EnemyTerrainInput* terrains,
    std::uint32_t terrain_count,
    const EnemyContextInput* contexts,
    std::uint32_t context_count,
    const std::uint32_t* enemy_criterion_keys,
    std::uint32_t enemy_criterion_key_count,
    const std::uint16_t* enemy_group_offsets,
    std::uint32_t enemy_group_count,
    std::uint32_t scratch_group_count,
    const SpecialRuleInput* rule_rows,
    std::uint32_t rule_row_count,
    const std::uint16_t* rule_criterion_keys,
    std::uint32_t rule_criterion_key_count,
    const std::uint16_t* rule_group_offsets,
    std::uint32_t rule_group_count,
    std::uint32_t* output_seeds,
    std::uint64_t* output_trials,
    std::uint64_t output_capacity,
    std::uint64_t* output_stage_counts,
    std::uint32_t stage_count) {
    const std::uint32_t combined_enemy_group_count =
        enemy_group_count + scratch_group_count;
    const std::uint32_t expected_stage_count = 1u +
        (has_terrain_constraint != 0u ? 1u : 0u) +
        enemy_group_count + rule_group_count;
    if (values == nullptr || value_count == 0u || start_index > stop_index ||
        stop_index - start_index > 8000000u || low16_stride == 0u ||
        (low16_stride & 1u) == 0u || draw_index == 0u || draw_index > 64u ||
        playthrough == 0u || playthrough > 5u || filtered_terrain_rows == nullptr ||
        filtered_terrain_count == 0u || terrain_row_count == 0u ||
        allowed_terrain_rows == nullptr || has_terrain_constraint > 1u ||
        descriptor_thresholds == nullptr || enemy_rows == nullptr ||
        enemy_row_count == 0u ||
        enemy_row_count > native_enemy_matcher::kMaximumEnemyRows ||
        terrains == nullptr || terrain_count != terrain_row_count ||
        contexts == nullptr || context_count == 0u ||
        combined_enemy_group_count > native_enemy_matcher::kMaximumCriteriaGroups ||
        (combined_enemy_group_count != 0u &&
            (enemy_criterion_keys == nullptr || enemy_criterion_key_count == 0u ||
             enemy_group_offsets == nullptr || enemy_group_offsets[0] != 0u ||
             enemy_group_offsets[combined_enemy_group_count] != enemy_criterion_key_count)) ||
        rule_group_count > native_enemy_matcher::kMaximumCriteriaGroups ||
        (rule_group_count != 0u &&
            (scratch_group_count == 0u || rule_rows == nullptr || rule_row_count == 0u ||
             rule_criterion_keys == nullptr || rule_criterion_key_count == 0u ||
             rule_group_offsets == nullptr || rule_group_offsets[0] != 0u ||
             rule_group_offsets[rule_group_count] != rule_criterion_key_count)) ||
        output_seeds == nullptr || output_trials == nullptr || output_capacity == 0u ||
        output_stage_counts == nullptr || stage_count != expected_stage_count) {
        return UINT64_MAX;
    }
    if (start_index == stop_index) {
        for (std::uint32_t index = 0u; index < stage_count; ++index) {
            output_stage_counts[index] = 0u;
        }
        return 0u;
    }
    for (std::uint32_t index = 0u; index < filtered_terrain_count; ++index) {
        if (filtered_terrain_rows[index] >= terrain_row_count) {
            return UINT64_MAX;
        }
    }
    for (std::uint32_t group = 0u; group < combined_enemy_group_count; ++group) {
        if (enemy_group_offsets[group] >= enemy_group_offsets[group + 1u]) {
            return UINT64_MAX;
        }
    }
    for (std::uint32_t group = 0u; group < rule_group_count; ++group) {
        if (rule_group_offsets[group] >= rule_group_offsets[group + 1u]) {
            return UINT64_MAX;
        }
    }

    const AuxiliaryMatcherView view{
        playthrough,
        mode_threshold,
        filtered_terrain_rows,
        filtered_terrain_count,
        terrain_row_count,
        allowed_terrain_rows,
        descriptor_thresholds,
        selector_threshold,
        role_five_threshold,
        selector_value,
        enemy_rows,
        enemy_row_count,
        terrains,
        terrain_count,
        contexts,
        context_count,
        enemy_criterion_keys,
        enemy_group_offsets,
        enemy_group_count,
        scratch_group_count,
        rule_rows,
        rule_row_count,
        rule_criterion_keys,
        rule_group_offsets,
        rule_group_count,
    };
    int device_count = 0;
    if (cudaGetDeviceCount(&device_count) == cudaSuccess && device_count > 0 &&
        collect_auxiliary_pivot_matches_on_cuda(
            values,
            value_count,
            start_index,
            stop_index,
            low16_stride,
            draw_index,
            view,
            has_terrain_constraint != 0u,
            output_seeds,
            output_trials,
            output_capacity,
            output_stage_counts,
            stage_count)) {
        g_last_backend = 1;
        std::uint64_t match_count = output_stage_counts[stage_count - 1u];
        if (enemy_group_count == 0u && rule_group_count == 0u &&
            has_terrain_constraint == 0u) {
            match_count = output_stage_counts[0];
        }
        return match_count;
    }

    for (std::uint32_t index = 0u; index < stage_count; ++index) {
        output_stage_counts[index] = 0u;
    }
    std::uint64_t output_count = 0u;
    for (std::uint64_t flat_index = start_index; flat_index < stop_index; ++flat_index) {
        const std::uint64_t low_index = flat_index / value_count;
        const std::uint32_t bucket_index =
            static_cast<std::uint32_t>(flat_index % value_count);
        const std::uint16_t low16 = static_cast<std::uint16_t>(
            static_cast<std::uint32_t>(low_index) * low16_stride);
        const std::uint32_t rotation =
            static_cast<std::uint32_t>(low_index % value_count);
        const std::uint16_t high16 = values[(rotation + bucket_index) % value_count];
        std::uint32_t seed = (static_cast<std::uint32_t>(high16) << 16u) | low16;
        for (std::uint32_t draw = 0u; draw < draw_index; ++draw) {
            seed = kLcgInverse * (seed - 1u);
        }
        if (!is_natural_seed(seed)) {
            continue;
        }
        ++output_stage_counts[0];
        const AuxiliaryEvaluation result = evaluate_auxiliary_constraints(seed, view);
        std::uint32_t stage_index = 1u;
        if (has_terrain_constraint != 0u) {
            if (!result.terrain_matches) {
                continue;
            }
            ++output_stage_counts[stage_index++];
        }
        bool accepted = result.terrain_matches;
        for (std::uint32_t group = 0u; accepted && group < enemy_group_count; ++group) {
            const std::uint32_t prefix = target_group_mask(group + 1u);
            accepted = (result.enemy_mask & prefix) == prefix;
            if (accepted) {
                ++output_stage_counts[stage_index];
            }
            ++stage_index;
        }
        for (std::uint32_t group = 0u; accepted && group < rule_group_count; ++group) {
            const std::uint32_t prefix = target_group_mask(group + 1u);
            accepted = (result.rule_mask & prefix) == prefix;
            if (accepted) {
                ++output_stage_counts[stage_index];
            }
            ++stage_index;
        }
        if (!accepted) {
            continue;
        }
        if (output_count >= output_capacity) {
            return UINT64_MAX;
        }
        output_seeds[output_count] = seed;
        output_trials[output_count] = flat_index + 1u;
        ++output_count;
    }
    g_last_backend = 0;
    return output_count;
}

extern "C" __declspec(dllexport) std::uint64_t collect_natural_pivot_seeds(
    const std::uint16_t* values,
    std::uint32_t value_count,
    std::uint64_t start_index,
    std::uint64_t stop_index,
    std::uint16_t low16_stride,
    std::uint32_t draw_index,
    std::uint32_t* output_seeds,
    std::uint64_t* output_trials,
    std::uint64_t output_capacity) {
    if (values == nullptr || value_count == 0u || start_index > stop_index ||
        low16_stride == 0u || (low16_stride & 1u) == 0u || draw_index == 0u ||
        draw_index > 64u) {
        return UINT64_MAX;
    }
    int device_count = 0;
    if (cudaGetDeviceCount(&device_count) == cudaSuccess && device_count > 0) {
        const std::uint64_t cuda_result = collect_on_cuda(
            values,
            value_count,
            start_index,
            stop_index,
            low16_stride,
            draw_index,
            output_seeds,
            output_trials,
            output_capacity);
        if (cuda_result != UINT64_MAX) {
            g_last_backend = 1;
            return cuda_result;
        }
    }
    g_last_backend = 0;
    return collect_on_cpu(
        values,
        value_count,
        start_index,
        stop_index,
        low16_stride,
        draw_index,
        output_seeds,
        output_trials,
        output_capacity);
}
