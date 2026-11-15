// Cross-vendor seed search. Preserve the CUDA/domain RNG, float32 rounding,
// row order, grouped criteria, and exclusive cursor bounds exactly.
StructuredBuffer<uint> Data : register(t0);
StructuredBuffer<uint2> NaturalMatches : register(t1);
RWStructuredBuffer<uint2> Results : register(u0);
RWStructuredBuffer<uint> Counters : register(u1);
cbuffer DispatchParams : register(b0) { uint base_index; uint item_count; uint output_capacity; uint input_base; };
static const uint kLcgInverse = 0xA5E2A705u;
static const uint kLcgMultiplier = 0x00010DCDu;
static const uint kAuxiliaryModeSeedMaskLow = 0x01E3C78Fu;
static const uint kAuxiliaryModeSeedMaskHigh = 0x00E1C387u;
struct EnemyCandidateInput { uint lookup_key; float cost; uint scratch_rule_key; uint terrain_mask; uint playthrough_mask; uint linked_group; uint selector; uint role; uint terrain_class; uint parameter_type; };
struct EnemyTerrainInput { uint marker_2c; uint flags_2e; uint terrain_class; };
struct EnemyContextInput { uint mode; uint branch_class; float budgets[5]; };
struct SpecialRuleInput { uint key; uint weight; float cost; uint conflict_identity_0; uint conflict_identity_1; uint active_conflict_mask; uint scratch_bit; uint reserved; };
EnemyCandidateInput get_enemy(uint index) {
 uint p=Data[17]+10u*index; EnemyCandidateInput r;
 r.lookup_key=Data[p]; r.cost=asfloat(Data[p+1]); r.scratch_rule_key=Data[p+2]; r.terrain_mask=Data[p+3]; r.playthrough_mask=Data[p+4]; r.linked_group=Data[p+5]; r.selector=Data[p+6]; r.role=Data[p+7]; r.terrain_class=Data[p+8]; r.parameter_type=Data[p+9]; return r;
}
EnemyTerrainInput get_terrain(uint index) { uint p=Data[19]+3u*index; EnemyTerrainInput r; r.marker_2c=Data[p];r.flags_2e=Data[p+1];r.terrain_class=Data[p+2];return r; }
EnemyContextInput get_context(uint index) { uint p=Data[21]+7u*index; EnemyContextInput r;r.mode=Data[p];r.branch_class=Data[p+1];[unroll]for(uint i=0;i<5;i++)r.budgets[i]=asfloat(Data[p+2+i]);return r; }
SpecialRuleInput get_rule(uint index) { uint p=Data[27]+8u*index;SpecialRuleInput r;r.key=Data[p];r.weight=Data[p+1];r.cost=asfloat(Data[p+2]);r.conflict_identity_0=Data[p+3];r.conflict_identity_1=Data[p+4];r.active_conflict_mask=Data[p+5];r.scratch_bit=Data[p+6];r.reserved=Data[p+7];return r; }
uint lcg_step(uint state) { return kLcgMultiplier*state+1u; }
bool is_natural_seed(uint seed) { return (seed&0xF0000000u)==0u && (seed&65535u)!=0u; }
uint derive_auxiliary_mode_seed(uint seed) { return ((seed&kAuxiliaryModeSeedMaskLow)<<3u)|((seed>>4u)&kAuxiliaryModeSeedMaskHigh); }
uint derive_terrain_seed(uint seed) { return ((seed>>14u)&0x3FFFu)|((seed&0x3FFFu)<<14u); }
uint random_index_from_u16(uint value,uint count) { return ((value*(count&65535u))>>16u)+value*(count>>16u); }
int roll_10000_from_u16(uint value) { precise float unit=(float)value*(1.0f/65536.0f);precise float product=unit*10000.0f;return min((int)product,9999); }
uint generate_terrain_row_index(uint seed) {
 uint state=lcg_step(derive_auxiliary_mode_seed(seed));uint branch=2u;
 if(roll_10000_from_u16(state>>16u)>=asint(Data[7])) { state=lcg_step(state);branch=random_index_from_u16(state>>16u,2u)==0u?1u:0u; }
 uint draw=lcg_step(derive_terrain_seed(seed))>>16u;
 return branch==2u?random_index_from_u16(draw,Data[10]):Data[Data[8]+random_index_from_u16(draw,Data[9])];
}


static const uint kMissingParameterType = 0xFFu;
static const uint kMaximumEnemyRows = 512u;
static const uint kMaximumCriteriaGroups = 32u;
static const uint kMaximumReplacementRows = 64u;
static const uint kMaximumGroupSelections = 16u;

float subtract_f32(float left, float right) { precise float result = left - right; return result; }

uint descriptor_seed(uint seed) {
    return ((seed & kAuxiliaryModeSeedMaskHigh) << 4u) |
        ((seed >> 3u) & kAuxiliaryModeSeedMaskLow);
}

int active_budget_count(EnemyContextInput context) {
    int count = 0;
    while (count < 5 && context.budgets[count] > 0.0f) {
        ++count;
    }
    return count;
}

int choose_context(
    uint displayed_seed,
    int mode_threshold,
    uint context_count) {
    uint state = derive_auxiliary_mode_seed(displayed_seed);
    state = lcg_step(state);
    const int first_roll = roll_10000_from_u16(
        ((state >> 16u) & 65535u));
    uint branch_class = 2u;
    if (first_roll >= mode_threshold) {
        state = lcg_step(state);
        branch_class = random_index_from_u16(
            ((state >> 16u) & 65535u), 2u) == 0u ? 1u : 0u;
    }
    uint matching_count = 0u;
    for (uint index = 0u; index < context_count; ++index) {
        if (get_context(index).branch_class == branch_class) {
            ++matching_count;
        }
    }
    if (matching_count == 0u) {
        return -1;
    }
    state = lcg_step(state);
    uint selected = random_index_from_u16(
        ((state >> 16u) & 65535u), matching_count);
    for (uint index = 0u; index < context_count; ++index) {
        if (get_context(index).branch_class != branch_class) {
            continue;
        }
        if (selected == 0u) {
            return (int)(index);
        }
        --selected;
    }
    return -1;
}

struct DescriptorState {
    uint selector;
    bool flag_21;
    bool flag_22;
    bool flag_23;
};

DescriptorState generate_descriptor(
    uint displayed_seed,
    uint branch_class,
    int selector_threshold,
    uint selector_value) {
    uint state = descriptor_seed(displayed_seed);
    bool flags[3] = {false, false, false};
    for (int index = 0; index < 3; ++index) {
        state = lcg_step(state);
        flags[index] = asint(Data[Data[13] + (index)]) > roll_10000_from_u16(
            ((state >> 16u) & 65535u));
    }
    uint selector = 0u;
    if (branch_class != 0u) {
        state = lcg_step(state);
        if (selector_threshold > roll_10000_from_u16(
                ((state >> 16u) & 65535u))) {
            selector = selector_value;
        }
    }
    DescriptorState result; result.selector = selector; result.flag_21 = flags[0]; result.flag_22 = flags[1]; result.flag_23 = flags[2]; return result;
}

bool terrain_gate_accepts(
    EnemyCandidateInput row,
    EnemyTerrainInput terrain) {
    for (int bit = 0; bit < 3; ++bit) {
        if ((row.terrain_mask & (1u << bit)) == 0u) {
            continue;
        }
        const bool blocked = bit == 0
            ? terrain.marker_2c != 0u
            : ((terrain.flags_2e >> (bit - 1)) & 1u) != 0u;
        if (blocked) {
            return false;
        }
    }
    return true;
}

bool parameter_gate_accepts(
    EnemyCandidateInput row,
    bool descriptor_flag_22) {
    if (row.parameter_type == kMissingParameterType) {
        return true;
    }
    const bool neutral = row.parameter_type == 0u || row.parameter_type == 3u;
    if (descriptor_flag_22) {
        return neutral;
    }
    if (!neutral) {
        return true;
    }
    return row.role == 4u || row.role == 5u;
}

bool base_candidate_accepts(
    EnemyCandidateInput row,
    uint playthrough,
    EnemyTerrainInput terrain,
    DescriptorState descriptor) {
    if ((row.playthrough_mask & (1u << (playthrough - 1u))) == 0u) {
        return false;
    }
    if (descriptor.selector != 0u && row.role != 4u && row.role != 5u &&
        row.selector != descriptor.selector) {
        return false;
    }
    if (descriptor.flag_21 && row.terrain_class != 0u &&
        row.terrain_class != terrain.terrain_class) {
        return false;
    }
    return parameter_gate_accepts(row, descriptor.flag_22) &&
        terrain_gate_accepts(row, terrain);
}

bool pool_role_accepts(
    uint role,
    int pool_kind) {
    switch (pool_kind) {
        case 0: return role == 4u;
        case 1: return role == 5u;
        case 2: return role != 4u && role != 5u;
        case 3: return role != 4u && role != 5u && role != 0u && role != 2u;
        case 4: return role != 4u && role != 5u && (role == 0u || role == 2u);
        default: return false;
    }
}

void mark_enemy_constraints(
    uint lookup_key,
    uint group_count,
    inout uint matched_mask) {
    for (uint group = 0u; group < group_count; ++group) {
        if ((matched_mask & (1u << group)) != 0u) {
            continue;
        }
        for (uint index = Data[Data[24] + (group)];
             index < Data[Data[24] + (group + 1u)]; ++index) {
            if (Data[Data[23] + (index)] == lookup_key) {
                matched_mask |= 1u << group;
                break;
            }
        }
    }
}

int select_ticket_candidate(
    uint row_count,
    uint playthrough,
    EnemyTerrainInput terrain,
    DescriptorState descriptor,
    int pool_kind,
    float budget,
    uint ticket) {
    uint count = 0u;
    for (uint index = 0u; index < row_count; ++index) {
        EnemyCandidateInput row = get_enemy(index);
        if (base_candidate_accepts(row, playthrough, terrain, descriptor) &&
            pool_role_accepts(row.role, pool_kind) && row.cost <= budget) {
            ++count;
        }
    }
    if (count == 0u) {
        return -1;
    }
    uint selected = ticket % count;
    for (uint index = 0u; index < row_count; ++index) {
        EnemyCandidateInput row = get_enemy(index);
        if (!base_candidate_accepts(row, playthrough, terrain, descriptor) ||
            !pool_role_accepts(row.role, pool_kind) || row.cost > budget) {
            continue;
        }
        if (selected == 0u) {
            return (int)(index);
        }
        --selected;
    }
    return -1;
}

uint build_replacements(
    int selected_index,
    uint row_count,
    uint playthrough,
    EnemyTerrainInput terrain,
    DescriptorState descriptor,
    inout uint replacements[kMaximumReplacementRows]) {
    if (selected_index < 0) {
        return 0u;
    }
    replacements[0] = ((selected_index) & 65535u);
    uint count = 1u;
    const uint linked_group = get_enemy(selected_index).linked_group;
    if (linked_group == 0u) {
        return count;
    }
    for (uint index = 0u; index < row_count; ++index) {
        if ((int)(index) == selected_index) {
            continue;
        }
        EnemyCandidateInput row = get_enemy(index);
        if (count < kMaximumReplacementRows && row.linked_group == linked_group &&
            base_candidate_accepts(row, playthrough, terrain, descriptor) &&
            pool_role_accepts(row.role, 2)) {
            replacements[count++] = ((index) & 65535u);
        }
    }
    return count;
}

int mapped_general_row(
    uint source_index,
    inout uint role_02_ordinal,
    uint replacements[kMaximumReplacementRows],
    uint replacement_count) {
    EnemyCandidateInput source = get_enemy(source_index);
    if ((source.role == 0u || source.role == 2u) && replacement_count != 0u) {
        const int mapped = replacements[role_02_ordinal % replacement_count];
        ++role_02_ordinal;
        return mapped;
    }
    return (int)(source_index);
}

int choose_budget_candidate(
    uint row_count,
    uint playthrough,
    EnemyTerrainInput terrain,
    DescriptorState descriptor,
    int pool_kind,
    float remaining,
    bool prefer_maximum_cost,
    uint local_state,
    uint replacements[kMaximumReplacementRows],
    uint replacement_count) {
    float maximum_cost = -1.0f;
    if (prefer_maximum_cost) {
        uint role_02_ordinal = 0u;
        for (uint source_index = 0u; source_index < row_count; ++source_index) {
            EnemyCandidateInput source = get_enemy(source_index);
            if (!base_candidate_accepts(source, playthrough, terrain, descriptor) ||
                !pool_role_accepts(source.role, pool_kind)) {
                continue;
            }
            const int mapped_index = pool_kind == 2
                ? mapped_general_row(
                    source_index,
                    role_02_ordinal,
                    replacements,
                    replacement_count)
                : (int)(source_index);
            const float cost = get_enemy(mapped_index).cost;
            if (cost <= remaining && cost > maximum_cost) {
                maximum_cost = cost;
            }
        }
    }

    uint count = 0u;
    uint role_02_ordinal = 0u;
    for (uint source_index = 0u; source_index < row_count; ++source_index) {
        EnemyCandidateInput source = get_enemy(source_index);
        if (!base_candidate_accepts(source, playthrough, terrain, descriptor) ||
            !pool_role_accepts(source.role, pool_kind)) {
            continue;
        }
        const int mapped_index = pool_kind == 2
            ? mapped_general_row(
                source_index,
                role_02_ordinal,
                replacements,
                replacement_count)
            : (int)(source_index);
        const float cost = get_enemy(mapped_index).cost;
        if (cost <= remaining && (!prefer_maximum_cost || cost == maximum_cost)) {
            ++count;
        }
    }
    if (count == 0u) {
        return -1;
    }
    uint selected = (local_state >> 16u) % count;
    role_02_ordinal = 0u;
    for (uint source_index = 0u; source_index < row_count; ++source_index) {
        EnemyCandidateInput source = get_enemy(source_index);
        if (!base_candidate_accepts(source, playthrough, terrain, descriptor) ||
            !pool_role_accepts(source.role, pool_kind)) {
            continue;
        }
        const int mapped_index = pool_kind == 2
            ? mapped_general_row(
                source_index,
                role_02_ordinal,
                replacements,
                replacement_count)
            : (int)(source_index);
        const float cost = get_enemy(mapped_index).cost;
        if (cost > remaining || (prefer_maximum_cost && cost != maximum_cost)) {
            continue;
        }
        if (selected == 0u) {
            return mapped_index;
        }
        --selected;
    }
    return -1;
}

void run_budget_helper(
    uint row_count,
    uint playthrough,
    EnemyTerrainInput terrain,
    DescriptorState descriptor,
    int pool_kind,
    float budget,
    inout uint rng_state,
    bool first_group,
    uint replacements[kMaximumReplacementRows],
    uint replacement_count,
    uint group_count,
    inout uint matched_mask) {
    rng_state = lcg_step(rng_state);
    uint local_state = ((rng_state >> 16u) & 65535u);
    float remaining = budget;
    if (remaining <= 0.0f) {
        return;
    }
    while (remaining > 0.0f) {
        local_state = lcg_step(local_state);
        const int selected = choose_budget_candidate(
            row_count,
            playthrough,
            terrain,
            descriptor,
            pool_kind,
            remaining,
            first_group,
            local_state,
            replacements,
            replacement_count);
        if (selected < 0) {
            return;
        }
        mark_enemy_constraints(
            get_enemy(selected).lookup_key,
            group_count,
            matched_mask);
        if (first_group) {
            return;
        }
        remaining = subtract_f32(remaining, get_enemy(selected).cost);
    }
}

bool match_enemy_constraints_for_seed(
    uint displayed_seed,
    uint terrain_row_index,
    uint playthrough,
    int mode_threshold,
    int selector_threshold,
    int role_five_threshold,
    uint selector_value,
    uint row_count,
    uint terrain_count,
    uint context_count,
    uint group_count,
    out uint observed_mask) {
    observed_mask = 0u;
    if (terrain_row_index >= terrain_count) {
        return false;
    }
    const int context_index = choose_context(
        displayed_seed,
        mode_threshold,
        context_count);
    if (context_index < 0) {
        return group_count == 0u;
    }
    EnemyContextInput context = get_context(context_index);
    EnemyTerrainInput terrain = get_terrain(terrain_row_index);
    const DescriptorState descriptor = generate_descriptor(
        displayed_seed,
        context.branch_class,
        selector_threshold,
        selector_value);
    const uint target_mask = group_count == 32u
        ? 0xFFFFFFFFu
        : ((1u << group_count) - 1u);
    uint matched_mask = 0u;
    uint rng_state = displayed_seed & 0x0FFFFFFFu;
    const int active_count = active_budget_count(context);

    if (context.branch_class == 0u) {
        uint selected_rows[16] = (uint[16])0;
        uint blocked_groups[8] = (uint[8])0;
        const int highest = active_count - 1;
        for (int group_index = highest; group_index >= 0; --group_index) {
            bool use_role_five = true;
            bool has_role_five = false;
            for (uint index = 0u; index < row_count; ++index) {
                EnemyCandidateInput row = get_enemy(index);
                if (!((selected_rows[index >> 5u] & (1u << (index & 31u))) != 0u) && !((blocked_groups[row.linked_group >> 5u] & (1u << (row.linked_group & 31u))) != 0u) &&
                    row.role == 5u &&
                    base_candidate_accepts(row, playthrough, terrain, descriptor)) {
                    has_role_five = true;
                    break;
                }
            }
            use_role_five = has_role_five;
            if (has_role_five && group_index != highest) {
                rng_state = lcg_step(rng_state);
                use_role_five = roll_10000_from_u16(
                    ((rng_state >> 16u) & 65535u)) >= role_five_threshold;
            }

            rng_state = lcg_step(rng_state);
            uint local_state = ((rng_state >> 16u) & 65535u);
            float remaining = context.budgets[group_index];
            bool prefer_maximum = true;
            uint group_selected[kMaximumGroupSelections] = (uint[kMaximumGroupSelections])0;
            uint group_selected_count = 0u;
            while (remaining > 0.0f) {
                float maximum_cost = -1.0f;
                uint count = 0u;
                for (uint index = 0u; index < row_count; ++index) {
                    EnemyCandidateInput row = get_enemy(index);
                    if (((selected_rows[index >> 5u] & (1u << (index & 31u))) != 0u) ||
                        (row.linked_group != 0u && ((blocked_groups[row.linked_group >> 5u] & (1u << (row.linked_group & 31u))) != 0u)) ||
                        row.role != (use_role_five ? 5u : 4u) ||
                        !base_candidate_accepts(row, playthrough, terrain, descriptor) ||
                        row.cost > remaining) {
                        continue;
                    }
                    if (prefer_maximum && row.cost > maximum_cost) {
                        maximum_cost = row.cost;
                    }
                }
                for (uint index = 0u; index < row_count; ++index) {
                    EnemyCandidateInput row = get_enemy(index);
                    if (((selected_rows[index >> 5u] & (1u << (index & 31u))) != 0u) ||
                        (row.linked_group != 0u && ((blocked_groups[row.linked_group >> 5u] & (1u << (row.linked_group & 31u))) != 0u)) ||
                        row.role != (use_role_five ? 5u : 4u) ||
                        !base_candidate_accepts(row, playthrough, terrain, descriptor) ||
                        row.cost > remaining ||
                        (prefer_maximum && row.cost != maximum_cost)) {
                        continue;
                    }
                    ++count;
                }
                if (count == 0u) {
                    break;
                }
                local_state = lcg_step(local_state);
                uint selected_ordinal = (local_state >> 16u) % count;
                int selected = -1;
                for (uint index = 0u; index < row_count; ++index) {
                    EnemyCandidateInput row = get_enemy(index);
                    if (((selected_rows[index >> 5u] & (1u << (index & 31u))) != 0u) ||
                        (row.linked_group != 0u && ((blocked_groups[row.linked_group >> 5u] & (1u << (row.linked_group & 31u))) != 0u)) ||
                        row.role != (use_role_five ? 5u : 4u) ||
                        !base_candidate_accepts(row, playthrough, terrain, descriptor) ||
                        row.cost > remaining ||
                        (prefer_maximum && row.cost != maximum_cost)) {
                        continue;
                    }
                    if (selected_ordinal == 0u) {
                        selected = (int)(index);
                        break;
                    }
                    --selected_ordinal;
                }
                if (selected < 0) {
                    break;
                }
                mark_enemy_constraints(
                    get_enemy(selected).lookup_key,
                    group_count,
                    matched_mask);
                if (group_selected_count < kMaximumGroupSelections) {
                    group_selected[group_selected_count++] =
                        ((selected) & 65535u);
                }
                remaining = subtract_f32(remaining, get_enemy(selected).cost);
                prefer_maximum = false;
            }
            for (uint index = 0u; index < group_selected_count; ++index) {
                const uint selected = group_selected[index];
                selected_rows[selected >> 5u] |= 1u << (selected & 31u);
                const uint linked_group = get_enemy(selected).linked_group;
                if (linked_group != 0u) {
                    blocked_groups[linked_group >> 5u] |= 1u << (linked_group & 31u);
                }
            }
        }
        observed_mask = matched_mask;
        return matched_mask == target_mask;
    }

    const int first_group_index = context.branch_class == 1u
        ? active_count - 1
        : 0;
    const int last_group_index = context.branch_class == 1u
        ? -1
        : active_count;
    const int group_step = context.branch_class == 1u ? -1 : 1;
    for (int group_index = first_group_index;
         group_index != last_group_index;
         group_index += group_step) {
        float remaining = context.budgets[group_index];
        uint replacements[kMaximumReplacementRows] = (uint[kMaximumReplacementRows])0;
        uint replacement_count = 0u;
        const bool highest_class1 =
            context.branch_class == 1u && group_index == active_count - 1;
        if (!highest_class1 && !descriptor.flag_23) {
            rng_state = lcg_step(rng_state);
            const int first = select_ticket_candidate(
                row_count,
                playthrough,
                terrain,
                descriptor,
                3,
                remaining,
                ((rng_state >> 16u) & 65535u));
            if (first >= 0) {
                mark_enemy_constraints(
                    get_enemy(first).lookup_key,
                    group_count,
                    matched_mask);
                remaining = subtract_f32(remaining, get_enemy(first).cost);
            }
            rng_state = lcg_step(rng_state);
            const int second = select_ticket_candidate(
                row_count,
                playthrough,
                terrain,
                descriptor,
                4,
                remaining,
                ((rng_state >> 16u) & 65535u));
            replacement_count = build_replacements(
                second,
                row_count,
                playthrough,
                terrain,
                descriptor,
                replacements);
        }
        run_budget_helper(
            row_count,
            playthrough,
            terrain,
            descriptor,
            highest_class1 ? 1 : 2,
            remaining,
            rng_state,
            highest_class1,
            replacements,
            replacement_count,
            group_count,
            matched_mask);
    }
    observed_mask = matched_mask;
    return matched_mask == target_mask;
}

bool special_rule_conflicts(
    SpecialRuleInput current,
    SpecialRuleInput previous) {
    const uint current_identities[2] = {
        current.conflict_identity_0,
        current.conflict_identity_1,
    };
    const uint previous_identities[2] = {
        previous.conflict_identity_0,
        previous.conflict_identity_1,
    };
    for (uint current_index = 0u; current_index < 2u; ++current_index) {
        if ((current.active_conflict_mask & (1u << current_index)) == 0u) {
            continue;
        }
        const uint identity = current_identities[current_index];
        if (identity == 0xFFFFu) {
            continue;
        }
        for (uint previous_index = 0u; previous_index < 2u;
             ++previous_index) {
            if (identity == previous_identities[previous_index]) {
                return true;
            }
        }
    }
    return false;
}

void mark_special_rule_constraints(
    uint key,
    uint group_count,
    inout uint matched_mask) {
    for (uint group = 0u; group < group_count; ++group) {
        if ((matched_mask & (1u << group)) != 0u) {
            continue;
        }
        for (uint index = Data[Data[30] + (group)];
             index < Data[Data[30] + (group + 1u)]; ++index) {
            if (Data[Data[29] + (index)] == key) {
                matched_mask |= 1u << group;
                break;
            }
        }
    }
}

uint match_special_rules_for_seed(
    uint displayed_seed,
    uint scratch_mask,
    uint row_count,
    uint group_count) {
    uint state = displayed_seed & 0x0FFFFFFFu;
    state = lcg_step(state);
    const int target_budget = (int)(
        random_index_from_u16(((state >> 16u) & 65535u), 5u)) + 1;
    float remaining = (float)(target_budget);
    const float original_budget = remaining;
    uint selected_keys[3] = (uint[3])0;
    uint selected_rows[3] = (uint[3])0;
    uint selected_count = 0u;
    bool zero_selected = false;
    float third_slot_best_abs = 0.0f;
    uint matched_mask = 0u;

    for (uint attempt = 0u; attempt < 3u; ++attempt) {
        uint total_weight = 0u;
        for (uint index = 0u; index < row_count; ++index) {
            SpecialRuleInput row = get_rule(index);
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
                    for (uint selected = 0u;
                         selected < selected_count; ++selected) {
                        if (row.key == selected_keys[selected] ||
                            special_rule_conflicts(row, get_rule(selected_rows[selected]))) {
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
            total_weight = ((
                total_weight + row.weight) & 65535u);
        }
        if (total_weight == 0u) {
            break;
        }

        state = lcg_step(state);
        uint ticket = random_index_from_u16(
            ((state >> 16u) & 65535u), total_weight);
        int chosen_index = -1;
        for (uint index = 0u; index < row_count; ++index) {
            SpecialRuleInput row = get_rule(index);
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
                    for (uint selected = 0u;
                         selected < selected_count; ++selected) {
                        if (row.key == selected_keys[selected] ||
                            special_rule_conflicts(row, get_rule(selected_rows[selected]))) {
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
                chosen_index = (int)(index);
                break;
            }
            ticket -= row.weight;
        }
        if (chosen_index < 0) {
            break;
        }
        SpecialRuleInput chosen = get_rule(chosen_index);
        selected_keys[selected_count] = chosen.key;
        selected_rows[selected_count] = ((chosen_index) & 65535u);
        ++selected_count;
        if (chosen.key == 0u) {
            zero_selected = true;
        } else {
            mark_special_rule_constraints(
                chosen.key,
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


uint target_group_mask(uint count) { return count==32u?0xFFFFFFFFu:(count==0u?0u:((1u<<count)-1u)); }
struct AuxiliaryEvaluation { uint terrain_row;bool terrain_matches;uint enemy_mask;uint rule_mask; };
AuxiliaryEvaluation evaluate_auxiliary(uint seed) {
 AuxiliaryEvaluation r=(AuxiliaryEvaluation)0;r.terrain_row=generate_terrain_row_index(seed);
 r.terrain_matches=r.terrain_row<Data[10] && Data[Data[11]+r.terrain_row]!=0u;
 if(!r.terrain_matches)return r;
 uint combined=0u;if(Data[25]+Data[26]!=0u)match_enemy_constraints_for_seed(seed,r.terrain_row,Data[12],asint(Data[7]),asint(Data[14]),asint(Data[15]),Data[16],Data[18],Data[20],Data[22],Data[25]+Data[26],combined);
 r.enemy_mask=combined&target_group_mask(Data[25]);if(r.enemy_mask!=target_group_mask(Data[25]))return r;
 if(Data[31]!=0u){uint scratch=Data[25]==32u?0u:((combined>>Data[25])&target_group_mask(Data[26]));r.rule_mask=match_special_rules_for_seed(seed,scratch,Data[28],Data[31]);}return r;
}
bool auxiliary_accepts(AuxiliaryEvaluation r) { return r.terrain_matches && r.enemy_mask==target_group_mask(Data[25]) && r.rule_mask==target_group_mask(Data[31]); }
uint pivot_seed(uint flat) { uint count=Data[2];uint low=flat/count;uint bucket=flat%count;uint high=Data[Data[1]+((low%count+bucket)%count)];uint seed=(high<<16u)|((low*Data[3])&65535u);[loop]for(uint d=0;d<Data[4];d++)seed=kLcgInverse*(seed-1u);return seed; }
uint primary_id(uint seed,bool r4) {
 uint state=seed;uint context=0u;
 if(r4){state=lcg_step(state);context=Data[Data[33]+(state>>16u)];}else{[loop]for(uint d=0;d<Data[40];d++)state=lcg_step(state);}
 state=lcg_step(state);bool promoted=Data[Data[36]+(state>>16u)]!=0u;bool primary_promoted=false;
 if(promoted){uint order[7]={0,1,2,3,4,5,6};[unroll]for(uint p=0;p<7;p++){state=lcg_step(state);uint other=Data[Data[37]+(state>>16u)];uint tmp=order[p];order[p]=order[other];order[other]=tmp;}
 [unroll]for(uint p=0;p<7;p++){uint selected=order[p];uint limit=r4?5u:Data[41];uint excluded=r4?1u:Data[42];if(selected>=limit || (excluded&(1u<<selected))!=0u)continue;primary_promoted=selected==(r4?1u:Data[43]);break;}}
 state=lcg_step(state);uint lookup=context*65536u+(state>>16u);return Data[Data[primary_promoted?35:34]+lookup];
}
void append_match(uint seed,uint flat) { uint index;InterlockedAdd(Counters[0],1u,index);if(index<output_capacity)Results[index]=uint2(seed,flat); }
[numthreads(256,1,1)]void CollectNatural(uint3 id:SV_DispatchThreadID) { if(id.x>=item_count)return;uint flat=base_index+id.x;uint seed=pivot_seed(flat);if(is_natural_seed(seed))append_match(seed,flat); }
[numthreads(256,1,1)]void CollectR4(uint3 id:SV_DispatchThreadID) { if(id.x>=item_count)return;uint flat=base_index+id.x;uint seed=pivot_seed(flat);if(!is_natural_seed(seed))return;uint effect=primary_id(seed,true);[loop]for(uint i=0;i<Data[39];i++){if(effect==Data[Data[38]+i]){append_match(seed,flat);return;}} }
[numthreads(64,1,1)]void CollectAuxiliary(uint3 id:SV_DispatchThreadID) {
 if(id.x>=item_count)return;uint2 natural=NaturalMatches[base_index+id.x];uint seed=natural.x;uint flat=natural.y;
 uint ignored;InterlockedAdd(Counters[1],1u,ignored);AuxiliaryEvaluation r=evaluate_auxiliary(seed);uint stage=2u;
 if(Data[32]!=0u){if(!r.terrain_matches)return;InterlockedAdd(Counters[stage++],1u,ignored);}
 [loop]for(uint g=0;g<Data[25];g++){uint mask=target_group_mask(g+1u);if((r.enemy_mask&mask)!=mask)return;InterlockedAdd(Counters[stage++],1u,ignored);}
 [loop]for(uint g=0;g<Data[31];g++){uint mask=target_group_mask(g+1u);if((r.rule_mask&mask)!=mask)return;InterlockedAdd(Counters[stage++],1u,ignored);}
 if(auxiliary_accepts(r))append_match(seed,flat);
}
[numthreads(256,1,1)]void FilterPrimary(uint3 id:SV_DispatchThreadID) { if(id.x>=item_count)return;uint index=input_base+id.x;Results[index]=uint2(primary_id(Data[Data[5]+index],false),0u); }
[numthreads(64,1,1)]void FilterAuxiliary(uint3 id:SV_DispatchThreadID) { if(id.x>=item_count)return;uint index=input_base+id.x;Results[index]=uint2(auxiliary_accepts(evaluate_auxiliary(Data[Data[5]+index]))?1u:0u,0u); }
[numthreads(1,1,1)]void SeedSelfTest(uint3 id:SV_DispatchThreadID) { Results[0]=uint2(lcg_step(0x12345678u),kLcgInverse*(lcg_step(0x12345678u)-1u));Counters[0]=1u; }
