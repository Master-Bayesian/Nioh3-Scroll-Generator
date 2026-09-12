
supplied-text-section: file format binary


Disassembly of section .data:

00000000001b2f34 <.data>:
  1b2f34:	48 89 5c 24 10                                  	mov    QWORD PTR [rsp+0x10],rbx
  1b2f39:	48 89 74 24 18                                  	mov    QWORD PTR [rsp+0x18],rsi
  1b2f3e:	55                                              	push   rbp
  1b2f3f:	57                                              	push   rdi
  1b2f40:	41 56                                           	push   r14
  1b2f42:	48 8d ac 24 a0 f5 ff ff                         	lea    rbp,[rsp-0xa60]
  1b2f4a:	48 81 ec 60 0b 00 00                            	sub    rsp,0xb60
  1b2f51:	48 8b 05 d8 0f 30 04                            	mov    rax,QWORD PTR [rip+0x4300fd8]        # 0x44b3f30
  1b2f58:	48 33 c4                                        	xor    rax,rsp
  1b2f5b:	48 89 85 50 0a 00 00                            	mov    QWORD PTR [rbp+0xa50],rax
  1b2f62:	33 f6                                           	xor    esi,esi
  1b2f64:	48 8b f9                                        	mov    rdi,rcx
  1b2f67:	8b 89 f8 e8 00 00                               	mov    ecx,DWORD PTR [rcx+0xe8f8]
  1b2f6d:	8d 5e 07                                        	lea    ebx,[rsi+0x7]
  1b2f70:	3b cb                                           	cmp    ecx,ebx
  1b2f72:	0f 8f a4 01 00 00                               	jg     0x1b311c
  1b2f78:	0f 84 88 01 00 00                               	je     0x1b3106
  1b2f7e:	83 e9 01                                        	sub    ecx,0x1
  1b2f81:	0f 84 54 01 00 00                               	je     0x1b30db
  1b2f87:	83 e9 01                                        	sub    ecx,0x1
  1b2f8a:	0f 84 bf 00 00 00                               	je     0x1b304f
  1b2f90:	83 e9 01                                        	sub    ecx,0x1
  1b2f93:	0f 84 a9 00 00 00                               	je     0x1b3042
  1b2f99:	83 e9 01                                        	sub    ecx,0x1
  1b2f9c:	0f 84 93 00 00 00                               	je     0x1b3035
  1b2fa2:	83 e9 01                                        	sub    ecx,0x1
  1b2fa5:	74 67                                           	je     0x1b300e
  1b2fa7:	83 f9 01                                        	cmp    ecx,0x1
  1b2faa:	0f 85 7e 03 00 00                               	jne    0x1b332e
  1b2fb0:	8b 87 fc e8 00 00                               	mov    eax,DWORD PTR [rdi+0xe8fc]
  1b2fb6:	83 e8 0c                                        	sub    eax,0xc
  1b2fb9:	3b c1                                           	cmp    eax,ecx
  1b2fbb:	0f 87 6d 03 00 00                               	ja     0x1b332e
  1b2fc1:	e8 b6 03 00 00                                  	call   0x1b337c
  1b2fc6:	84 c0                                           	test   al,al
  1b2fc8:	74 14                                           	je     0x1b2fde
  1b2fca:	e8 c5 f3 61 02                                  	call   0x27d2394
  1b2fcf:	c7 87 f8 e8 00 00 09 00 00 00                   	mov    DWORD PTR [rdi+0xe8f8],0x9
  1b2fd9:	e9 50 03 00 00                                  	jmp    0x1b332e
  1b2fde:	8b 87 b8 00 00 00                               	mov    eax,DWORD PTR [rdi+0xb8]
  1b2fe4:	85 c0                                           	test   eax,eax
  1b2fe6:	74 12                                           	je     0x1b2ffa
  1b2fe8:	3b c3                                           	cmp    eax,ebx
  1b2fea:	b9 0a 00 00 00                                  	mov    ecx,0xa
  1b2fef:	0f 45 d9                                        	cmovne ebx,ecx
  1b2ff2:	89 9f f8 e8 00 00                               	mov    DWORD PTR [rdi+0xe8f8],ebx
  1b2ff8:	eb 0a                                           	jmp    0x1b3004
  1b2ffa:	c7 87 f8 e8 00 00 0c 00 00 00                   	mov    DWORD PTR [rdi+0xe8f8],0xc
  1b3004:	e8 4f 03 00 00                                  	call   0x1b3358
  1b3009:	e9 20 03 00 00                                  	jmp    0x1b332e
  1b300e:	48 8d 4f 10                                     	lea    rcx,[rdi+0x10]
  1b3012:	c7 87 f8 e8 00 00 06 00 00 00                   	mov    DWORD PTR [rdi+0xe8f8],0x6
  1b301c:	c7 87 fc e8 00 00 06 00 00 00                   	mov    DWORD PTR [rdi+0xe8fc],0x6
  1b3026:	e8 25 37 14 00                                  	call   0x2f6750
  1b302b:	e8 74 03 00 00                                  	call   0x1b33a4
  1b3030:	e9 f9 02 00 00                                  	jmp    0x1b332e
  1b3035:	48 8b cf                                        	mov    rcx,rdi
  1b3038:	e8 3f 0f e3 01                                  	call   0x1fe3f7c
  1b303d:	e9 ec 02 00 00                                  	jmp    0x1b332e
  1b3042:	48 8b cf                                        	mov    rcx,rdi
  1b3045:	e8 46 10 e3 01                                  	call   0x1fe4090
  1b304a:	e9 df 02 00 00                                  	jmp    0x1b332e
  1b304f:	83 bf fc e8 00 00 0c                            	cmp    DWORD PTR [rdi+0xe8fc],0xc
  1b3056:	0f 85 d2 02 00 00                               	jne    0x1b332e
  1b305c:	40 38 b7 26 0e 00 00                            	cmp    BYTE PTR [rdi+0xe26],sil
  1b3063:	75 3f                                           	jne    0x1b30a4
  1b3065:	40 38 b7 27 0e 00 00                            	cmp    BYTE PTR [rdi+0xe27],sil
  1b306c:	74 14                                           	je     0x1b3082
  1b306e:	39 b7 d8 0d 00 00                               	cmp    DWORD PTR [rdi+0xdd8],esi
  1b3074:	0f 94 c0                                        	sete   al
  1b3077:	88 87 24 0e 00 00                               	mov    BYTE PTR [rdi+0xe24],al
  1b307d:	e9 a2 02 00 00                                  	jmp    0x1b3324
  1b3082:	40 38 b7 25 0e 00 00                            	cmp    BYTE PTR [rdi+0xe25],sil
  1b3089:	0f 84 95 02 00 00                               	je     0x1b3324
  1b308f:	48 8b cf                                        	mov    rcx,rdi
  1b3092:	e8 31 03 00 00                                  	call   0x1b33c8
  1b3097:	84 c0                                           	test   al,al
  1b3099:	0f 85 85 02 00 00                               	jne    0x1b3324
  1b309f:	e9 79 02 00 00                                  	jmp    0x1b331d
  1b30a4:	39 b7 d8 0d 00 00                               	cmp    DWORD PTR [rdi+0xdd8],esi
  1b30aa:	75 20                                           	jne    0x1b30cc
  1b30ac:	c7 87 b8 00 00 00 02 00 00 00                   	mov    DWORD PTR [rdi+0xb8],0x2
  1b30b6:	c7 87 f8 e8 00 00 0c 00 00 00                   	mov    DWORD PTR [rdi+0xe8f8],0xc
  1b30c0:	c7 87 fc e8 00 00 0d 00 00 00                   	mov    DWORD PTR [rdi+0xe8fc],0xd
  1b30ca:	eb 2c                                           	jmp    0x1b30f8
  1b30cc:	c7 87 f8 e8 00 00 03 00 00 00                   	mov    DWORD PTR [rdi+0xe8f8],0x3
  1b30d6:	e9 53 02 00 00                                  	jmp    0x1b332e
  1b30db:	48 8b 4f 08                                     	mov    rcx,QWORD PTR [rdi+0x8]
  1b30df:	c7 87 f8 e8 00 00 02 00 00 00                   	mov    DWORD PTR [rdi+0xe8f8],0x2
  1b30e9:	c7 87 fc e8 00 00 02 00 00 00                   	mov    DWORD PTR [rdi+0xe8fc],0x2
  1b30f3:	e8 28 ba e8 ff                                  	call   0x3eb20
  1b30f8:	48 8d 4f 10                                     	lea    rcx,[rdi+0x10]
  1b30fc:	e8 4f 36 14 00                                  	call   0x2f6750
  1b3101:	e9 28 02 00 00                                  	jmp    0x1b332e
  1b3106:	c7 87 f8 e8 00 00 08 00 00 00                   	mov    DWORD PTR [rdi+0xe8f8],0x8
  1b3110:	c7 87 fc e8 00 00 08 00 00 00                   	mov    DWORD PTR [rdi+0xe8fc],0x8
  1b311a:	eb dc                                           	jmp    0x1b30f8
  1b311c:	83 e9 08                                        	sub    ecx,0x8
  1b311f:	0f 84 ef 01 00 00                               	je     0x1b3314
  1b3125:	83 e9 01                                        	sub    ecx,0x1
  1b3128:	0f 84 d1 01 00 00                               	je     0x1b32ff
  1b312e:	83 e9 01                                        	sub    ecx,0x1
  1b3131:	74 32                                           	je     0x1b3165
  1b3133:	83 e9 01                                        	sub    ecx,0x1
  1b3136:	74 11                                           	je     0x1b3149
  1b3138:	83 f9 01                                        	cmp    ecx,0x1
  1b313b:	0f 85 ed 01 00 00                               	jne    0x1b332e
  1b3141:	40 8a f1                                        	mov    sil,cl
  1b3144:	e9 e5 01 00 00                                  	jmp    0x1b332e
  1b3149:	e8 2e ea f3 ff                                  	call   0xf1b7c
  1b314e:	84 c0                                           	test   al,al
  1b3150:	0f 85 d8 01 00 00                               	jne    0x1b332e
  1b3156:	c7 87 f8 e8 00 00 0c 00 00 00                   	mov    DWORD PTR [rdi+0xe8f8],0xc
  1b3160:	e9 c9 01 00 00                                  	jmp    0x1b332e
  1b3165:	48 8d 4c 24 60                                  	lea    rcx,[rsp+0x60]
  1b316a:	e8 71 0e 55 00                                  	call   0x703fe0
  1b316f:	83 bf b8 00 00 00 06                            	cmp    DWORD PTR [rdi+0xb8],0x6
  1b3176:	c7 44 24 60 01 00 00 00                         	mov    DWORD PTR [rsp+0x60],0x1
  1b317e:	0f 85 07 01 00 00                               	jne    0x1b328b
  1b3184:	33 d2                                           	xor    edx,edx
  1b3186:	48 8d 8d 50 02 00 00                            	lea    rcx,[rbp+0x250]
  1b318d:	41 b8 00 08 00 00                               	mov    r8d,0x800
  1b3193:	e8 38 0c 9e 00                                  	call   0xb93dd0
  1b3198:	b9 f0 3e f5 03                                  	mov    ecx,0x3f53ef0
  1b319d:	e8 92 18 3f 00                                  	call   0x5a4a34
  1b31a2:	b9 00 04 00 00                                  	mov    ecx,0x400
  1b31a7:	48 89 75 30                                     	mov    QWORD PTR [rbp+0x30],rsi
  1b31ab:	48 8b d8                                        	mov    rbx,rax
  1b31ae:	48 89 4d 38                                     	mov    QWORD PTR [rbp+0x38],rcx
  1b31b2:	48 8d 85 50 02 00 00                            	lea    rax,[rbp+0x250]
  1b31b9:	48 89 4d 28                                     	mov    QWORD PTR [rbp+0x28],rcx
  1b31bd:	48 89 45 18                                     	mov    QWORD PTR [rbp+0x18],rax
  1b31c1:	4c 8d 35 70 65 74 03                            	lea    r14,[rip+0x3746570]        # 0x38f9738
  1b31c8:	48 8d 85 50 02 00 00                            	lea    rax,[rbp+0x250]
  1b31cf:	48 89 75 20                                     	mov    QWORD PTR [rbp+0x20],rsi
  1b31d3:	b9 30 f6 ff 03                                  	mov    ecx,0x3fff630
  1b31d8:	48 89 45 40                                     	mov    QWORD PTR [rbp+0x40],rax
  1b31dc:	4c 89 75 10                                     	mov    QWORD PTR [rbp+0x10],r14
  1b31e0:	e8 4f 18 3f 00                                  	call   0x5a4a34
  1b31e5:	48 89 44 24 30                                  	mov    QWORD PTR [rsp+0x30],rax
  1b31ea:	48 8b ce                                        	mov    rcx,rsi
  1b31ed:	0f 10 44 24 30                                  	movups xmm0,XMMWORD PTR [rsp+0x30]
  1b31f2:	48 8d 44 24 40                                  	lea    rax,[rsp+0x40]
  1b31f7:	48 89 5c 24 20                                  	mov    QWORD PTR [rsp+0x20],rbx
  1b31fc:	0f 10 4c 24 20                                  	movups xmm1,XMMWORD PTR [rsp+0x20]
  1b3201:	48 89 44 24 28                                  	mov    QWORD PTR [rsp+0x28],rax
  1b3206:	48 8d 05 2b 94 75 03                            	lea    rax,[rip+0x375942b]        # 0x390c638
  1b320d:	48 89 44 24 30                                  	mov    QWORD PTR [rsp+0x30],rax
  1b3212:	f3 0f 7f 44 24 40                               	movdqu XMMWORD PTR [rsp+0x40],xmm0
  1b3218:	48 c7 44 24 20 cc 00 00 00                      	mov    QWORD PTR [rsp+0x20],0xcc
  1b3221:	f3 0f 7f 4c 24 50                               	movdqu XMMWORD PTR [rsp+0x50],xmm1
  1b3227:	48 ff c1                                        	inc    rcx
  1b322a:	48 8d 40 02                                     	lea    rax,[rax+0x2]
  1b322e:	66 39 30                                        	cmp    WORD PTR [rax],si
  1b3231:	75 f4                                           	jne    0x1b3227
  1b3233:	0f 28 44 24 20                                  	movaps xmm0,XMMWORD PTR [rsp+0x20]
  1b3238:	4c 8d 44 24 20                                  	lea    r8,[rsp+0x20]
  1b323d:	48 89 4c 24 38                                  	mov    QWORD PTR [rsp+0x38],rcx
  1b3242:	48 8d 54 24 30                                  	lea    rdx,[rsp+0x30]
  1b3247:	0f 28 4c 24 30                                  	movaps xmm1,XMMWORD PTR [rsp+0x30]
  1b324c:	48 8d 4d 10                                     	lea    rcx,[rbp+0x10]
  1b3250:	66 0f 7f 4c 24 30                               	movdqa XMMWORD PTR [rsp+0x30],xmm1
  1b3256:	66 0f 7f 44 24 20                               	movdqa XMMWORD PTR [rsp+0x20],xmm0
  1b325c:	e8 27 b0 15 00                                  	call   0x30e288
  1b3261:	b9 ff 03 00 00                                  	mov    ecx,0x3ff
  1b3266:	4c 89 75 10                                     	mov    QWORD PTR [rbp+0x10],r14
  1b326a:	48 3b c1                                        	cmp    rax,rcx
  1b326d:	48 0f 47 c1                                     	cmova  rax,rcx
  1b3271:	48 8d 4d 10                                     	lea    rcx,[rbp+0x10]
  1b3275:	66 89 b4 45 50 02 00 00                         	mov    WORD PTR [rbp+rax*2+0x250],si
  1b327d:	e8 96 4f 80 00                                  	call   0x9b8218
  1b3282:	48 8d 95 50 02 00 00                            	lea    rdx,[rbp+0x250]
  1b3289:	eb 0d                                           	jmp    0x1b3298
  1b328b:	b9 30 f6 ff 03                                  	mov    ecx,0x3fff630
  1b3290:	e8 9f 17 3f 00                                  	call   0x5a4a34
  1b3295:	48 8b d0                                        	mov    rdx,rax
  1b3298:	48 8d 4c 24 68                                  	lea    rcx,[rsp+0x68]
  1b329d:	e8 ca d5 15 00                                  	call   0x31086c
  1b32a2:	b9 31 85 c6 03                                  	mov    ecx,0x3c68531
  1b32a7:	e8 88 17 3f 00                                  	call   0x5a4a34
  1b32ac:	48 8b d0                                        	mov    rdx,rax
  1b32af:	48 8d 4d 88                                     	lea    rcx,[rbp-0x78]
  1b32b3:	e8 b4 d5 15 00                                  	call   0x31086c
  1b32b8:	48 8d 54 24 60                                  	lea    rdx,[rsp+0x60]
  1b32bd:	c7 45 f0 4a b4 2d 72                            	mov    DWORD PTR [rbp-0x10],0x722db44a
  1b32c4:	48 8d 4d 10                                     	lea    rcx,[rbp+0x10]
  1b32c8:	c7 45 ec 38 75 c9 37                            	mov    DWORD PTR [rbp-0x14],0x37c97538
  1b32cf:	c7 45 e8 2d a4 d7 86                            	mov    DWORD PTR [rbp-0x18],0x86d7a42d
  1b32d6:	e8 d5 5b 55 00                                  	call   0x708eb0
  1b32db:	48 8b c8                                        	mov    rcx,rax
  1b32de:	0f 57 d2                                        	xorps  xmm2,xmm2
  1b32e1:	83 ca ff                                        	or     edx,0xffffffff
  1b32e4:	e8 97 0c 55 00                                  	call   0x703f80
  1b32e9:	48 8d 4c 24 60                                  	lea    rcx,[rsp+0x60]
  1b32ee:	c7 87 f8 e8 00 00 0b 00 00 00                   	mov    DWORD PTR [rdi+0xe8f8],0xb
  1b32f8:	e8 e7 0f 55 00                                  	call   0x7042e4
  1b32fd:	eb 2f                                           	jmp    0x1b332e
  1b32ff:	e8 78 00 00 00                                  	call   0x1b337c
  1b3304:	84 c0                                           	test   al,al
  1b3306:	75 26                                           	jne    0x1b332e
  1b3308:	c7 87 f8 e8 00 00 06 00 00 00                   	mov    DWORD PTR [rdi+0xe8f8],0x6
  1b3312:	eb 1a                                           	jmp    0x1b332e
  1b3314:	83 bf fc e8 00 00 0c                            	cmp    DWORD PTR [rdi+0xe8fc],0xc
  1b331b:	75 11                                           	jne    0x1b332e
  1b331d:	c6 87 24 0e 00 00 01                            	mov    BYTE PTR [rdi+0xe24],0x1
  1b3324:	c7 87 f8 e8 00 00 05 00 00 00                   	mov    DWORD PTR [rdi+0xe8f8],0x5
  1b332e:	40 8a c6                                        	mov    al,sil
  1b3331:	48 8b 8d 50 0a 00 00                            	mov    rcx,QWORD PTR [rbp+0xa50]
  1b3338:	48 33 cc                                        	xor    rcx,rsp
  1b333b:	e8 70 de 9d 00                                  	call   0xb911b0
  1b3340:	4c 8d 9c 24 60 0b 00 00                         	lea    r11,[rsp+0xb60]
  1b3348:	49 8b 5b 28                                     	mov    rbx,QWORD PTR [r11+0x28]
  1b334c:	49 8b 73 30                                     	mov    rsi,QWORD PTR [r11+0x30]
  1b3350:	49 8b e3                                        	mov    rsp,r11
  1b3353:	41 5e                                           	pop    r14
  1b3355:	5f                                              	pop    rdi
  1b3356:	5d                                              	pop    rbp
  1b3357:	c3                                              	ret
