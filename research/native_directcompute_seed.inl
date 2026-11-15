// Included by the existing DirectCompute helper: share its device selection and
// buffer implementation, and keep CUDA optional for every seed-search operation.
#include <array>
#include <memory>
#include <map>
#include <mutex>
#include "seed_collect_natural.h"
#include "seed_collect_r4.h"
#include "seed_collect_auxiliary.h"
#include "seed_filter_primary.h"
#include "seed_filter_auxiliary.h"
#include "seed_self_test.h"

namespace {
constexpr std::uint32_t SEED_SEARCH_ABI = 1u;
constexpr std::uint32_t SEED_COUNTERS = 68u;
constexpr std::uint32_t SEED_DISPATCH_ITEMS = 65536u;
struct SeedDevice {
    DeviceBundle bundle;
    std::array<ComPtr<ID3D11ComputeShader>, 6> shaders;
    bool self_tested = false;
};
std::mutex seed_mutex;
std::map<std::uint32_t, std::unique_ptr<SeedDevice>> seed_devices;

SeedDevice* seed_device(std::uint32_t vendor, HRESULT* failure) {
    auto found = seed_devices.find(vendor);
    if (found != seed_devices.end()) {
        if (SUCCEEDED(found->second->bundle.device->GetDeviceRemovedReason())) return found->second.get();
        // A lost device owns no publishable window. Recreate and self-test on
        // the next request instead of advertising a permanently stale context.
        seed_devices.erase(found);
    }
    auto result = std::make_unique<SeedDevice>();
    if (!create_device(vendor, &result->bundle)) {
        *failure = DXGI_ERROR_NOT_FOUND;
        return nullptr;
    }
    struct ShaderBytes { const void* data; std::size_t size; };
    const ShaderBytes shaders[] = {
        {g_seed_collect_natural, sizeof(g_seed_collect_natural)},
        {g_seed_collect_r4, sizeof(g_seed_collect_r4)},
        {g_seed_collect_auxiliary, sizeof(g_seed_collect_auxiliary)},
        {g_seed_filter_primary, sizeof(g_seed_filter_primary)},
        {g_seed_filter_auxiliary, sizeof(g_seed_filter_auxiliary)},
        {g_seed_self_test, sizeof(g_seed_self_test)},
    };
    for (std::size_t i = 0; i < std::size(shaders); ++i) {
        *failure = result->bundle.device->CreateComputeShader(
            shaders[i].data, shaders[i].size, nullptr, &result->shaders[i]);
        if (FAILED(*failure)) return nullptr;
    }
    auto pointer = result.get();
    seed_devices.emplace(vendor, std::move(result));
    return pointer;
}

std::uint64_t run_seed_search(
    SeedDevice& device, const std::uint32_t* words, std::uint32_t word_count,
    std::uint64_t start, std::uint64_t stop, std::uint32_t capacity,
    std::uint32_t* output, std::uint64_t* stages, std::uint32_t stage_count,
    HRESULT* failure) {
    auto* gpu = device.bundle.device.Get();
    auto* context = device.bundle.context.Get();
    ComPtr<ID3D11Buffer> input, results, counters, constants, natural;
    ComPtr<ID3D11ShaderResourceView> input_view, natural_view;
    ComPtr<ID3D11UnorderedAccessView> result_view, counter_view, natural_output;
    if (!create_structured_buffer(gpu, words, word_count, &input, &input_view) ||
        !create_output_buffer(gpu, std::max(capacity, 1u), &results, &result_view) ||
        !create_output_u32_buffer(gpu, SEED_COUNTERS, &counters, &counter_view)) {
        *failure = E_OUTOFMEMORY;
        return ERROR_RESULT;
    }
    D3D11_BUFFER_DESC description{};
    description.ByteWidth = 16u;
    description.Usage = D3D11_USAGE_DEFAULT;
    description.BindFlags = D3D11_BIND_CONSTANT_BUFFER;
    *failure = gpu->CreateBuffer(&description, nullptr, &constants);
    if (FAILED(*failure)) return ERROR_RESULT;
    const UINT zeros[4] = {};
    ID3D11Buffer* uniform[] = {constants.Get()};
    context->CSSetConstantBuffers(0u, 1u, uniform);
    auto dispatch = [&](std::uint32_t operation, std::uint64_t first,
                        std::uint64_t last, std::uint32_t result_capacity,
                        ID3D11UnorderedAccessView* result) {
        context->ClearUnorderedAccessViewUint(counter_view.Get(), zeros);
        ID3D11ShaderResourceView* inputs[] = {input_view.Get(), natural_view.Get()};
        ID3D11UnorderedAccessView* outputs[] = {result, counter_view.Get()};
        context->CSSetShaderResources(0u, 2u, inputs);
        context->CSSetUnorderedAccessViews(0u, 2u, outputs, nullptr);
        context->CSSetShader(device.shaders[operation].Get(), nullptr, 0u);
        const std::uint32_t threads = operation == 2u || operation == 4u ? 64u : 256u;
        for (std::uint64_t base = first; base < last;) {
            const auto count = static_cast<std::uint32_t>(
                std::min<std::uint64_t>(last - base, SEED_DISPATCH_ITEMS));
            const std::uint32_t values[] = {
                static_cast<std::uint32_t>(base), count, result_capacity,
                static_cast<std::uint32_t>(base)};
            context->UpdateSubresource(constants.Get(), 0u, nullptr, values, 0u, 0u);
            context->Dispatch(operation == 5u ? 1u : (count + threads - 1u) / threads, 1u, 1u);
            base += count;
        }
        ID3D11UnorderedAccessView* no_outputs[] = {nullptr, nullptr};
        ID3D11ShaderResourceView* no_inputs[] = {nullptr, nullptr};
        context->CSSetUnorderedAccessViews(0u, 2u, no_outputs, nullptr);
        context->CSSetShaderResources(0u, 2u, no_inputs);
    };
    std::uint64_t dispatch_start = start, dispatch_stop = stop;
    if (words[0] == 2u) {
        // Compact natural seeds first, as CUDA does. Running the large enemy
        // matcher on sparse pivot threads wastes registers and integrated-GPU
        // bandwidth even when most threads return before evaluating a row.
        const auto natural_capacity = static_cast<std::uint32_t>(stop - start);
        description = {};
        description.ByteWidth = std::max(natural_capacity, 1u) * 8u;
        description.Usage = D3D11_USAGE_DEFAULT;
        description.BindFlags = D3D11_BIND_UNORDERED_ACCESS | D3D11_BIND_SHADER_RESOURCE;
        description.MiscFlags = D3D11_RESOURCE_MISC_BUFFER_STRUCTURED;
        description.StructureByteStride = 8u;
        *failure = gpu->CreateBuffer(&description, nullptr, &natural);
        if (FAILED(*failure)) return ERROR_RESULT;
        *failure = gpu->CreateUnorderedAccessView(natural.Get(), nullptr, &natural_output);
        if (FAILED(*failure)) return ERROR_RESULT;
        dispatch(0u, start, stop, natural_capacity, natural_output.Get());
        std::uint32_t natural_count = 0u;
        if (!read_buffer(gpu, context, counters.Get(), &natural_count, sizeof(natural_count)) ||
            natural_count > natural_capacity) {
            *failure = E_FAIL;
            return ERROR_RESULT;
        }
        *failure = gpu->CreateShaderResourceView(natural.Get(), nullptr, &natural_view);
        if (FAILED(*failure)) return ERROR_RESULT;
        dispatch_start = 0u;
        dispatch_stop = natural_count;
    }
    dispatch(words[0], dispatch_start, dispatch_stop, capacity, result_view.Get());
    std::uint32_t observed[SEED_COUNTERS] = {};
    if (!read_buffer(gpu, context, counters.Get(), observed, sizeof(observed))) {
        *failure = gpu->GetDeviceRemovedReason();
        if (SUCCEEDED(*failure)) *failure = E_FAIL;
        return ERROR_RESULT;
    }
    const std::uint64_t count = words[0] == 3u || words[0] == 4u
        ? stop - start : observed[0];
    if (count > capacity) {
        *failure = HRESULT_FROM_WIN32(ERROR_INSUFFICIENT_BUFFER);
        return ERROR_RESULT;
    }
    if (count != 0u && !read_buffer(gpu, context, results.Get(), output,
            static_cast<std::uint32_t>(count) * 2u * sizeof(std::uint32_t))) {
        *failure = gpu->GetDeviceRemovedReason();
        if (SUCCEEDED(*failure)) *failure = E_FAIL;
        return ERROR_RESULT;
    }
    for (std::uint32_t i = 0; i < stage_count; ++i) stages[i] = observed[i + 1u];
    *failure = S_OK;
    return count;
}

bool seed_self_test(SeedDevice& device, HRESULT* failure) {
    if (device.self_tested) return true;
    std::uint32_t words[48] = {5u};
    std::uint32_t output[2] = {};
    const auto count = run_seed_search(device, words, 48u, 0u, 1u, 1u,
        output, nullptr, 0u, failure);
    device.self_tested = count == 1u && output[0] == (0x10DCDu * 0x12345678u + 1u)
        && output[1] == 0x12345678u;
    if (!device.self_tested && SUCCEEDED(*failure)) *failure = E_FAIL;
    return device.self_tested;
}
} // namespace

extern "C" __declspec(dllexport) int d3d11_seed_search_abi_version() {
    return SEED_SEARCH_ABI;
}
extern "C" __declspec(dllexport) int d3d11_seed_search_available(std::uint32_t vendor) {
    std::lock_guard<std::mutex> lock(seed_mutex);
    HRESULT failure = S_OK;
    auto* device = seed_device(vendor, &failure);
    return device && seed_self_test(*device, &failure) ? 1 : 0;
}
extern "C" __declspec(dllexport) std::uint64_t execute_seed_search_d3d11(
    const std::uint32_t* words, std::uint32_t word_count,
    std::uint64_t start, std::uint64_t stop, std::uint32_t capacity,
    std::uint32_t* output, std::uint64_t* stages, std::uint32_t stage_count,
    std::uint32_t vendor, std::uint32_t* used_vendor, std::int32_t* error) {
    if (!error) return ERROR_RESULT;
    *error = E_INVALIDARG;
    if (!words || word_count < 48u || words[0] >= 5u || !output || !used_vendor ||
        start > stop || stop > 0x100000000ull || stop - start > 50000000ull ||
        capacity == 0u || capacity > 4000000u || stage_count >= SEED_COUNTERS ||
        (stage_count != 0u && !stages)) return ERROR_RESULT;
    std::lock_guard<std::mutex> lock(seed_mutex);
    HRESULT failure = S_OK;
    auto* device = seed_device(vendor, &failure);
    if (!device || !seed_self_test(*device, &failure)) {
        *error = failure;
        return ERROR_RESULT;
    }
    *used_vendor = device->bundle.vendor_id;
    const auto count = run_seed_search(*device, words, word_count, start, stop,
        capacity, output, stages, stage_count, &failure);
    *error = failure;
    return count;
}
