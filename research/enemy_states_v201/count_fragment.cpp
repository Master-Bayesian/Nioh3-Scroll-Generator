// Isolated raw E3A900 fragment differential harness. NOT a game injector.
#include <cstdint>
#include <cstring>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <vector>
#include <sys/mman.h>
struct MT {uint32_t i;uint32_t s[1248];uint32_t mask;};
struct Args {float p;uint32_t already;MT* mt;};
struct Input {uint32_t seed,n,pbits,already,bonus,repeat;};
int main(int argc,char**argv){
 if(argc!=2)return 2;
 constexpr size_t SIZE=0x4600000;
 auto mem=(uint8_t*)mmap(nullptr,SIZE,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANONYMOUS,-1,0);
 if(mem==MAP_FAILED)return 3;
 std::ifstream f(argv[1],std::ios::binary); std::vector<uint8_t>b((std::istreambuf_iterator<char>(f)),{});
 if(b.size()!=0x489)return 4;
 memcpy(mem+0xE3A900,b.data(),b.size());
 float one=1.0f, lim=9223372036854775808.0f;
 memcpy(mem+0x3BD00A0,&one,4);memcpy(mem+0x3BD01DC,&lim,4);
 // Native tail dereferences global -> root; only effect-presence helper is a
 // test stub. Preserve every count/RNG machine instruction.
 void* root=calloc(1,0x3000); void* outer=&root;
 memcpy(mem+0x45B5E00,&outer,8);
 // stub at E3AE00: mov eax, [rip+disp-to-testbonus]; ret
 const uint32_t STUB=0xE3AE00, TEST=0xE3AF00;
 mem[STUB]=0x8b;mem[STUB+1]=0x05;int32_t disp=TEST-(STUB+6);memcpy(mem+STUB+2,&disp,4);mem[STUB+6]=0xc3;
 int32_t rel=STUB-(0xE3AD62+5); if(mem[0xE3AD62]!=0xE8)return 5;memcpy(mem+0xE3AD63,&rel,4);
 // Only the function/stub/test-bonus page is executable. Other pages remain RW.
 // The small same-page test bonus is a harness input, not game state.
 if(mprotect(mem+0xE3A000,0x1000,PROT_READ|PROT_WRITE|PROT_EXEC))return 6;
 using Fn=int(__attribute__((ms_abi)) *)(Args*,uint64_t);
 Fn fn=(Fn)(mem+0xE3A900);Input in;
 while(fread(&in,sizeof(in),1,stdin)==1){
  MT mt{}; mt.i=624;mt.mask=0xffffffff;mt.s[0]=in.seed;
  for(uint32_t i=1;i<624;++i)mt.s[i]=1812433253u*(mt.s[i-1]^(mt.s[i-1]>>30))+i;
  Args a{};memcpy(&a.p,&in.pbits,4);a.already=in.already;a.mt=&mt;
  memcpy(mem+TEST,&in.bonus,4);int count=0;
  for(uint32_t r=0;r<in.repeat;++r)count=fn(&a,in.n);
  fwrite(&count,4,1,stdout);fwrite(&mt,sizeof(mt),1,stdout);
 }
 free(root);munmap(mem,SIZE);return 0;
}
