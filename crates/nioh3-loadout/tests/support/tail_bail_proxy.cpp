// Test-only NK_ABI 1 forwarding library. Corrupts one real tail result to prove verifier sensitivity.
#define _CRT_SECURE_NO_WARNINGS
#include <windows.h>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
using u8=uint8_t; using u32=uint32_t; using u64=uint64_t; using i32=int32_t;
#define API extern "C" __declspec(dllexport)
template<class F> F target(const char* name) {
    static HMODULE library=nullptr;
    if(!library) {const char* path=std::getenv("NIOH3_TEST_REAL_KERNEL");library=path?LoadLibraryA(path):nullptr;}
    auto function=library?GetProcAddress(library,name):nullptr;
    if(!function){std::fprintf(stderr,"Test forwarding library cannot resolve %s\n",name);std::exit(9);}
    return reinterpret_cast<F>(function);
}
API u32 nk_abi(){return target<u32(*)()>("nk_abi")();}
API u32 nk_item_size(){return target<u32(*)()>("nk_item_size")();}
API u32 nk_cand_slots(){return target<u32(*)()>("nk_cand_slots")();}
API void* nk_open(const void* files,u32 count,char* err,u32 length){return target<void*(*)(const void*,u32,char*,u32)>("nk_open")(files,count,err,length);}
API int nk_set_pool(void* h,const u8* p,u64 n,char* e,u32 l){return target<int(*)(void*,const u8*,u64,char*,u32)>("nk_set_pool")(h,p,n,e,l);}
API int nk_cases(void* h,const void* cases,u64 n,const void* limits,u32* bits,u8* outcomes,char* err,u32 length){
    int result=target<int(*)(void*,const void*,u64,const void*,u32*,u8*,char*,u32)>("nk_cases")(h,cases,n,limits,bits,outcomes,err,length);
    if(result&&n&&std::getenv("NIOH3_TEST_TAIL_BAIL")){outcomes[n-1]=2;bits[n-1]=0;}
    return result;
}
API int nk_search(void* h,const void* spaces,u32 ns,const u32* lists,u64 nl,const i32* attrs,const void* limits,float keep,u32 launch,void* hits,u32 capacity,u32* hit_n,u64* counts,u64* missed,u32 mc,u32* mn,const volatile u32* cancel,char* err,u32 len){
    return target<int(*)(void*,const void*,u32,const u32*,u64,const i32*,const void*,float,u32,void*,u32,u32*,u64*,u64*,u32,u32*,const volatile u32*,char*,u32)>("nk_search")(h,spaces,ns,lists,nl,attrs,limits,keep,launch,hits,capacity,hit_n,counts,missed,mc,mn,cancel,err,len);
}
API void nk_close(void* h){target<void(*)(void*)>("nk_close")(h);}
