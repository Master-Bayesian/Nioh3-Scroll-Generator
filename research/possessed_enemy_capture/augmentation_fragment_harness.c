/* Isolated Linux x86-64 experiment, NOT game execution or mission parity.
 * E39D40 original instructions except three rel32 calls retargeted to explicit
 * ms_abi database/map stubs. 13684C is executed byte-for-byte without patches.
 * No game process, game save, account data or network input is involved.
 */
#define _GNU_SOURCE
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
#define MS __attribute__((ms_abi))
#define CHECK(x) do {if(!(x)){fprintf(stderr,"check failed line %d: %s\n",__LINE__,#x);exit(2);}} while(0)

typedef struct {uint32_t key,lookup; uint8_t rest[12];} Desc;
typedef struct {Desc *begin,*end,*cap; uint8_t opaque[16];} Wave;
typedef struct {Wave *begin,*end,*cap;} Generated;
typedef struct {uint64_t links[2];uint32_t key,value;} Node;
typedef struct {Node *node;uint64_t added;} Result;
typedef struct {Node nodes[128];size_t count;} Acc;
typedef struct {Generated *p;Acc *map;uint32_t *counter;} Args;
typedef struct {uint32_t key,pad;uint64_t value;} Entry;
typedef struct {Entry *entries;uint64_t count,capacity;} Sorted;
static uint8_t rows[8+5*0x398];
static uint64_t context[5];
static unsigned nlookup,nmap;
static void *MS get_db(void){return context;}
static uint32_t MS lookup_row(void *h,uint32_t key){(void)h;nlookup++;return key<5?key:UINT32_MAX;}
static Result *MS lookup_map(Acc *m,Result *r,Desc *d){
 nmap++;size_t i;for(i=0;i<m->count;i++) if(m->nodes[i].key==d->key)break;
 if(i==m->count){CHECK(i<128);memset(&m->nodes[i],0,sizeof(Node));m->nodes[i].key=d->key;m->count++;r->added=1;}
 else r->added=0;
 r->node=&m->nodes[i];return r;
}
static uint32_t rndstate=0x938AB521;
static uint32_t rnd(void){rndstate^=rndstate<<13;rndstate^=rndstate>>17;rndstate^=rndstate<<5;return rndstate;}
static void load_code(uint8_t *dst,size_t size,const char *path){FILE*f=fopen(path,"rb");CHECK(f);CHECK(fread(dst,1,size,f)==size);CHECK(fgetc(f)==EOF);fclose(f);}
static void trampoline(uint8_t *mem,size_t site,size_t at,void *fn){
 CHECK(mem[site]==0xe8);int32_t displacement=(int32_t)(at-site-5);memcpy(mem+site+1,&displacement,4);
 mem[at]=0x48;mem[at+1]=0xb8;uint64_t addr=(uint64_t)(uintptr_t)fn;memcpy(mem+at+2,&addr,8);mem[at+10]=0xff;mem[at+11]=0xe0;
}
int main(int argc,char**argv){
 CHECK(argc==3);CHECK(sizeof(Desc)==20&&sizeof(Wave)==40&&sizeof(Node)==24&&sizeof(Entry)==16);
 uint8_t *mem=mmap(NULL,4096,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANONYMOUS,-1,0);CHECK(mem!=MAP_FAILED);
 load_code(mem,228,argv[1]);load_code(mem+512,86,argv[2]);
 trampoline(mem,0x56,256,(void*)get_db);trampoline(mem,0x65,272,(void*)lookup_row);trampoline(mem,0xa4,288,(void*)lookup_map);
 CHECK(mprotect(mem,4096,PROT_READ|PROT_EXEC)==0);
 typedef void (MS *Prepass)(Args*,uint8_t);
 typedef uint64_t(MS *Lookup)(Sorted*,uint32_t);
 Prepass pre=(Prepass)(void*)mem;Lookup lookup=(Lookup)(void*)(mem+512);
 context[0]=(uint64_t)(uintptr_t)rows;context[4]=0xA123;
 unsigned cases=0,total_desc=0;
 for(unsigned c=0;c<1024;c++){
  Desc ds[8][12];Wave waves[8];memset(ds,0,sizeof(ds));memset(waves,0,sizeof(waves));
  unsigned nw=rnd()%9;for(unsigned w=0;w<nw;w++){
   unsigned n=rnd()%13;waves[w].begin=ds[w];waves[w].end=ds[w]+n;waves[w].cap=ds[w]+12;
   for(unsigned j=0;j<n;j++){
    ds[w][j].key=rnd()%23;ds[w][j].lookup=rnd()%7;ds[w][j].rest[8]=(uint8_t)(rnd()%3);
   }total_desc+=n;
  }
  memset(rows,0,sizeof(rows));uint32_t nrows=5;memcpy(rows+4,&nrows,4);
  for(unsigned i=0;i<5;i++){uint32_t flags=rnd()&0xffff;memcpy(rows+8+i*0x398+0x74,&flags,4);}
  Desc before[8][12];Wave wb[8];memcpy(before,ds,sizeof(ds));memcpy(wb,waves,sizeof(waves));
  Generated p={waves,waves+nw,waves+8},pb=p;Acc a={0};
  uint32_t counter=(c%7==0)?0xfffffffe:0,expect=counter;uint32_t expected[23]={0};uint8_t present[23]={0};unsigned ec=0,lc=0,mc=0;
  for(unsigned cls=0;cls<2;cls++)for(unsigned w=0;w<nw;w++)for(Desc*d=waves[w].begin;d<waves[w].end;d++){
   if(d->rest[8]!=cls)continue;
   lc++;
   if(d->lookup>=5)continue;
   uint32_t flags;memcpy(&flags,rows+8+d->lookup*0x398+0x74,4);
   if(!(flags&0x4000))continue;
   mc++;
   if(!present[d->key])ec++;
   present[d->key]=1;expected[d->key]=expect++;
  }
  Args args={&p,&a,&counter};nlookup=nmap=0;pre(&args,0);pre(&args,1);
  CHECK(counter==expect&&a.count==ec&&nlookup==lc&&nmap==mc);
  CHECK(!memcmp(before,ds,sizeof(ds))&&!memcmp(wb,waves,sizeof(waves))&&!memcmp(&pb,&p,sizeof(p)));
  for(size_t i=0;i<a.count;i++)CHECK(present[a.nodes[i].key]&&a.nodes[i].value==expected[a.nodes[i].key]);
  cases++;
 }
 unsigned queries=0;
 for(unsigned n=0;n<=256;n++){
  Entry e[256];for(unsigned j=0;j<n;j++){e[j].key=(uint32_t)(((uint64_t)UINT32_MAX*j)/257);e[j].pad=0xBEEFCAFE;e[j].value=0x10000+j*0x100;}
  Sorted s={e,n,n};
  for(unsigned q=0;q<32;q++){
   uint32_t key=q<n&&q%2==0?e[q].key:rnd();uint64_t expected=0;
   for(unsigned j=0;j<n;j++)if(e[j].key==key){expected=e[j].value;break;}
   CHECK(lookup(&s,key)==expected);queries++;
  }
 }
 CHECK(munmap(mem,4096)==0);
 printf("{\"execution\":\"isolated_original_fragments\",\"game_runtime\":false,\"prepass_cases\":%u,\"descriptors_examined\":%u,\"lookup_queries\":%u,\"prepass_helper_calls_stubbed\":3,\"lookup_instruction_patches\":0,\"all_match\":true}\n",cases,total_desc,queries);
 return 0;
}
