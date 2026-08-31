#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <algorithm>
#include <cstdint>
#include <cstring>
#include <vector>

#include <d3d11.h>
#include <d3dcompiler.h>
#include <dxgi1_2.h>
#include <wrl/client.h>

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
#pragma pack(pop)

static_assert(sizeof(EffectPathInput) == 112u);

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

    ComPtr<ID3DBlob> shader_blob;
    ComPtr<ID3DBlob> error_blob;
    const UINT compile_flags = D3DCOMPILE_OPTIMIZATION_LEVEL3 | D3DCOMPILE_IEEE_STRICTNESS;
    if (FAILED(D3DCompile(
            SHADER_SOURCE,
            sizeof(SHADER_SOURCE) - 1u,
            "effect_preimage.hlsl",
            nullptr,
            nullptr,
            "main",
            "cs_5_0",
            compile_flags,
            0u,
            &shader_blob,
            &error_blob))) {
        return ERROR_RESULT;
    }
    ComPtr<ID3D11ComputeShader> shader;
    if (FAILED(bundle.device->CreateComputeShader(
            shader_blob->GetBufferPointer(),
            shader_blob->GetBufferSize(),
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
