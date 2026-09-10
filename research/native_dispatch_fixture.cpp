// Isolated Windows x64 debug-session fixture. Contains no game code or game APIs.
#include <windows.h>
#include <atomic>
#include <cstdio>
#include <cstring>
#include <thread>

int main() {
    auto code = static_cast<unsigned char*>(VirtualAlloc(nullptr, 4096, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE));
    if (!code) return 2;
    const unsigned char entry[] = {0x40,0x53,0x57,0x48,0x83,0xEC,0x38,0x48,0x83,0xC4,0x38,0x5F,0x5B,0xC3};
    const unsigned char caller[] = {0x48,0x83,0xEC,0x28,0x48,0xB8,0,0,0,0,0,0,0,0,0xFF,0xD0,0x48,0x83,0xC4,0x28,0xC3};
    memcpy(code, entry, sizeof entry);
    memcpy(code + 64, caller, sizeof caller);
    memcpy(code + 70, &code, sizeof code);
    FlushInstructionCache(GetCurrentProcess(), code, 4096);
    unsigned char data[512] = {}, queue[128] = {};
    void* data_pointer = data;
    void* manager_pointer = &data_pointer;
    printf("{\"pid\":%lu,\"entry\":%llu,\"caller_return\":%llu,\"manager_pointer\":%llu}\n",
           GetCurrentProcessId(), reinterpret_cast<unsigned long long>(code),
           reinterpret_cast<unsigned long long>(code + 80), reinterpret_cast<unsigned long long>(&manager_pointer));
    fflush(stdout);
    std::atomic<bool> stopped{false};
    std::thread input([&] { getchar(); stopped = true; });
    auto function = reinterpret_cast<void(*)(void*)>(code + 64);
    while (!stopped) { function(queue); Sleep(5); }
    input.join();
    VirtualFree(code, 0, MEM_RELEASE);
    return 0;
}
