// Compile against the production helper with MSVC AddressSanitizer.
// Private access stays in this test translation unit; no production API changes.
#include <algorithm>
#include <array>
#include <cstdio>
#include <cstring>
#include <stdexcept>
#include <string>
#include <vector>
#define private public
#include "CryptoState.h"
#undef private
extern "C" {
#include "aes.h"
}
// MSVC encodes access in symbols, so compile the same implementation here.
#include "CryptoState.cpp"

namespace {
constexpr unsigned char GUARD = 0xA5;
constexpr size_t GUARD_BYTES = 32;

void require(bool passed, const char* message) {
    if (!passed) throw std::runtime_error(message);
}

// Independent, bounded big-endian reference for expected output construction.
void reference_increment(std::array<unsigned char, BLOCK_SIZE>& counter) {
    for (auto byte = counter.rbegin(); byte != counter.rend(); ++byte) {
        if (++*byte != 0) return;
    }
}

void seed_keys(CryptoState& state) {
    for (size_t i = 0; i < BLOCK_SIZE; ++i) {
        state.s_key_1[i] = static_cast<unsigned char>(i * 3 + 1);
        state.s_key_2[i] = static_cast<unsigned char>(i * 5 + 2);
        state.s_IV_1[i] = static_cast<unsigned char>(i * 7 + 3);
        state.s_IV_2[i] = static_cast<unsigned char>(i * 11 + 4);
    }
}

std::vector<unsigned char> reference_region(const unsigned char* input, size_t length, const CryptoState& state) {
    std::vector<unsigned char> expected(length);
    std::array<unsigned char, BLOCK_SIZE> iv1{}, iv2{};
    std::copy_n(state.s_IV_1, BLOCK_SIZE, iv1.begin());
    std::copy_n(state.s_IV_2, BLOCK_SIZE, iv2.begin());
    for (size_t offset = 0; offset < length; offset += BLOCK_SIZE) {
        unsigned char stream1[BLOCK_SIZE], stream2[BLOCK_SIZE];
        AES_ECB_encrypt(iv1.data(), state.s_key_1, stream1, BLOCK_SIZE);
        AES_ECB_encrypt(iv2.data(), state.s_key_2, stream2, BLOCK_SIZE);
        reference_increment(iv1);
        reference_increment(iv2);
        const size_t count = std::min<size_t>(BLOCK_SIZE, length - offset);
        for (size_t j = 0; j < count; ++j) {
            expected[offset + j] = input[offset + j] ^ stream1[j] ^ stream2[j];
        }
    }
    return expected;
}

void counter_case(const char* label, std::array<unsigned char, BLOCK_SIZE> initial) {
    auto expected = initial;
    reference_increment(expected);
    std::vector<unsigned char> counter(initial.begin(), initial.end());
    CryptoState state;
    state.incr_byte_array(counter.data());
    require(std::equal(counter.begin(), counter.end(), expected.begin()), label);
    std::printf("PASS counter %s\n", label);
}

void counter_tests(bool only_wrap) {
    if (!only_wrap) {
        counter_case("ordinary", {});
        std::array<unsigned char, BLOCK_SIZE> one{};
        one[15] = 0xFF;
        counter_case("one-byte carry", one);
        one[14] = 0xFF;
        one[13] = 0x7F;
        counter_case("multi-byte carry", one);
    }
    std::array<unsigned char, BLOCK_SIZE> full;
    full.fill(0xFF);
    counter_case("full 128-bit wrap", full);
}

void check_guard(const std::vector<unsigned char>& output, size_t length, const char* message) {
    require(std::all_of(output.begin() + length, output.end(),
        [](unsigned char byte) { return byte == GUARD; }), message);
}

void header_tests() {
    for (bool padded : {true, false}) {
        // Padded input isolates the original tmp_clear stack overwrite.
        // Exact input also proves neither pass reads beyond the header.
        std::vector<unsigned char> input(HEADER_SIZE + (padded ? BLOCK_SIZE : 0));
        for (size_t i = 0; i < input.size(); ++i) input[i] = static_cast<unsigned char>(i * 13 + 9);
        const auto original = input;
        std::vector<unsigned char> output(HEADER_SIZE + GUARD_BYTES, GUARD);
        CryptoState state;
        seed_keys(state);
        state.savedata_encr = input.data();
        state.savedata_clear = output.data();
        const auto expected = reference_region(input.data(), HEADER_SIZE, state);
        state.decrypt_header();
        require(std::equal(expected.begin(), expected.end(), output.begin()), "header output differs");
        require(input == original, "header changed input");
        check_guard(output, HEADER_SIZE, "header wrote beyond output");
        std::printf("PASS header %s input, 344 bytes, partial final block 8 bytes\n", padded ? "padded" : "exact");
    }
}

void body_test(FILE_TYPE kind) {
    CryptoState::file_type = kind;
    const size_t body_bytes = kind == FILE_TYPE::USR ? USR_BODY_SIZE : SYS_BODY_SIZE;
    const size_t file_bytes = HEADER_SIZE + body_bytes;
    const size_t transformed = body_bytes / BLOCK_SIZE * BLOCK_SIZE;
    std::vector<unsigned char> input(file_bytes);
    for (size_t i = 0; i < input.size(); ++i) input[i] = static_cast<unsigned char>(i * 17 + 11);
    const auto original = input;
    std::vector<unsigned char> output(file_bytes + GUARD_BYTES, GUARD);
    CryptoState state;
    seed_keys(state);
    state.savedata_encr = input.data();
    state.savedata_clear = output.data();
    auto expected = reference_region(input.data() + HEADER_SIZE, transformed, state);
    expected.resize(body_bytes, 0);
    state.decrypt_body();
    require(std::equal(expected.begin(), expected.end(), output.begin() + HEADER_SIZE), "body output differs");
    require(input == original, "body changed input");
    require(std::all_of(output.begin(), output.begin() + HEADER_SIZE,
        [](unsigned char byte) { return byte == GUARD; }), "body changed header");
    check_guard(output, file_bytes, "body wrote beyond output");
    std::printf("PASS %s body: %zu bytes transformed, %zu trailer bytes zero, guards intact\n",
        kind == FILE_TYPE::USR ? "USR" : "SYS", transformed, body_bytes - transformed);
}
}

int main(int argc, char** argv) {
    std::setvbuf(stdout, nullptr, _IONBF, 0);
    const std::string selected = argc == 2 ? argv[1] : "all";
    try {
        if (selected == "all" || selected == "counter") counter_tests(false);
        else if (selected == "counter-wrap") counter_tests(true);
        if (selected == "all" || selected == "header") header_tests();
        if (selected == "all" || selected == "system-body") body_test(FILE_TYPE::SYS);
        if (selected == "all" || selected == "user-body") body_test(FILE_TYPE::USR);
        if (selected != "all" && selected != "counter" && selected != "counter-wrap" &&
            selected != "header" && selected != "system-body" && selected != "user-body") {
            throw std::runtime_error("unknown test case");
        }
        std::printf("PASS legacy crypto bounds regression (%s)\n", selected.c_str());
        return 0;
    } catch (const std::exception& error) {
        std::fprintf(stderr, "FAIL legacy crypto bounds regression: %s\n", error.what());
        return 1;
    }
}