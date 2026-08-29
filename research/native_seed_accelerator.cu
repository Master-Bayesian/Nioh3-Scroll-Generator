#include <cstddef>
#include <cstdint>

#include <cuda_runtime.h>

namespace {

constexpr std::uint32_t kLcgInverse = 0xA5E2A705u;
constexpr std::uint32_t kLcgMultiplier = 0x00010DCDu;
int g_last_backend = -1;

__host__ __device__ std::uint32_t lcg_step(std::uint32_t state) {
    return kLcgMultiplier * state + 1u;
}

__host__ __device__ bool is_natural_seed(std::uint32_t seed) {
    return (seed & 0xF0000000u) == 0u && (seed & 0xFFFFu) != 0u;
}

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

}  // namespace

extern "C" __declspec(dllexport) int cuda_seed_acceleration_available() {
    int device_count = 0;
    return cudaGetDeviceCount(&device_count) == cudaSuccess && device_count > 0 ? 1 : 0;
}

extern "C" __declspec(dllexport) int seed_accelerator_last_backend() {
    return g_last_backend;
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
