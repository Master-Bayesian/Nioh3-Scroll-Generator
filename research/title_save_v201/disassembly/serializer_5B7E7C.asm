
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b7e7c <.data>:
  5b7e7c:	48 8b c4                                        	mov    rax,rsp
  5b7e7f:	48 89 58 18                                     	mov    QWORD PTR [rax+0x18],rbx
  5b7e83:	55                                              	push   rbp
  5b7e84:	56                                              	push   rsi
  5b7e85:	57                                              	push   rdi
  5b7e86:	41 54                                           	push   r12
  5b7e88:	41 55                                           	push   r13
  5b7e8a:	41 56                                           	push   r14
  5b7e8c:	41 57                                           	push   r15
  5b7e8e:	48 8d 68 a1                                     	lea    rbp,[rax-0x5f]
  5b7e92:	48 81 ec f0 00 00 00                            	sub    rsp,0xf0
  5b7e99:	0f 29 70 b8                                     	movaps XMMWORD PTR [rax-0x48],xmm6
  5b7e9d:	48 8b 05 8c c0 ef 03                            	mov    rax,QWORD PTR [rip+0x3efc08c]        # 0x44b3f30
  5b7ea4:	48 33 c4                                        	xor    rax,rsp
  5b7ea7:	48 89 45 07                                     	mov    QWORD PTR [rbp+0x7],rax
  5b7eab:	48 8b fa                                        	mov    rdi,rdx
  5b7eae:	48 8b f1                                        	mov    rsi,rcx
  5b7eb1:	bb 58 01 00 00                                  	mov    ebx,0x158
  5b7eb6:	48 8b cf                                        	mov    rcx,rdi
  5b7eb9:	44 8b c3                                        	mov    r8d,ebx
  5b7ebc:	33 d2                                           	xor    edx,edx
  5b7ebe:	45 33 ed                                        	xor    r13d,r13d
  5b7ec1:	e8 0a bf 5d 00                                  	call   0xb93dd0
  5b7ec6:	44 38 ae 27 0e 00 00                            	cmp    BYTE PTR [rsi+0xe27],r13b
  5b7ecd:	48 8d 05 f4 8b 33 03                            	lea    rax,[rip+0x3338bf4]        # 0x38f0ac8
  5b7ed4:	4c 8d 05 a5 73 67 03                            	lea    r8,[rip+0x36773a5]        # 0x3c2f280
  5b7edb:	48 8b cf                                        	mov    rcx,rdi
  5b7ede:	4c 0f 44 c0                                     	cmove  r8,rax
  5b7ee2:	41 8d 55 08                                     	lea    edx,[r13+0x8]
  5b7ee6:	49 83 c9 ff                                     	or     r9,0xffffffffffffffff
  5b7eea:	e8 d5 28 60 00                                  	call   0xbba7c4
  5b7eef:	48 8b 86 d8 00 00 00                            	mov    rax,QWORD PTR [rsi+0xd8]
  5b7ef6:	33 c9                                           	xor    ecx,ecx
  5b7ef8:	48 89 47 08                                     	mov    QWORD PTR [rdi+0x8],rax
  5b7efc:	48 8b 86 b0 00 00 00                            	mov    rax,QWORD PTR [rsi+0xb0]
  5b7f03:	48 89 47 10                                     	mov    QWORD PTR [rdi+0x10],rax
  5b7f07:	89 5f 18                                        	mov    DWORD PTR [rdi+0x18],ebx
  5b7f0a:	8b 86 e8 00 00 00                               	mov    eax,DWORD PTR [rsi+0xe8]
  5b7f10:	89 47 1c                                        	mov    DWORD PTR [rdi+0x1c],eax
  5b7f13:	e8 48 de 60 00                                  	call   0xbc5d60
  5b7f18:	48 89 47 20                                     	mov    QWORD PTR [rdi+0x20],rax
  5b7f1c:	33 c9                                           	xor    ecx,ecx
  5b7f1e:	8b 86 04 01 00 00                               	mov    eax,DWORD PTR [rsi+0x104]
  5b7f24:	89 47 28                                        	mov    DWORD PTR [rdi+0x28],eax
  5b7f27:	48 8b 86 08 01 00 00                            	mov    rax,QWORD PTR [rsi+0x108]
  5b7f2e:	48 89 47 30                                     	mov    QWORD PTR [rdi+0x30],rax
  5b7f32:	8b 86 f0 00 00 00                               	mov    eax,DWORD PTR [rsi+0xf0]
  5b7f38:	89 47 38                                        	mov    DWORD PTR [rdi+0x38],eax
  5b7f3b:	8b 86 f8 00 00 00                               	mov    eax,DWORD PTR [rsi+0xf8]
  5b7f41:	89 87 8c 00 00 00                               	mov    DWORD PTR [rdi+0x8c],eax
  5b7f47:	8b 86 fc 00 00 00                               	mov    eax,DWORD PTR [rsi+0xfc]
  5b7f4d:	89 47 3c                                        	mov    DWORD PTR [rdi+0x3c],eax
  5b7f50:	0f b6 86 f4 00 00 00                            	movzx  eax,BYTE PTR [rsi+0xf4]
  5b7f57:	89 47 40                                        	mov    DWORD PTR [rdi+0x40],eax
  5b7f5a:	0f b6 86 10 01 00 00                            	movzx  eax,BYTE PTR [rsi+0x110]
  5b7f61:	89 47 44                                        	mov    DWORD PTR [rdi+0x44],eax
  5b7f64:	8a 86 00 01 00 00                               	mov    al,BYTE PTR [rsi+0x100]
  5b7f6a:	88 47 48                                        	mov    BYTE PTR [rdi+0x48],al
  5b7f6d:	8a 86 11 01 00 00                               	mov    al,BYTE PTR [rsi+0x111]
  5b7f73:	88 87 90 00 00 00                               	mov    BYTE PTR [rdi+0x90],al
  5b7f79:	e8 e2 dd 60 00                                  	call   0xbc5d60
  5b7f7e:	48 8b c8                                        	mov    rcx,rax
  5b7f81:	e8 96 f9 61 00                                  	call   0xbd791c
  5b7f86:	41 8b dd                                        	mov    ebx,r13d
  5b7f89:	41 bf ff 00 00 80                               	mov    r15d,0x800000ff
  5b7f8f:	41 be 00 ff ff ff                               	mov    r14d,0xffffff00
  5b7f95:	e8 56 f9 61 00                                  	call   0xbd78f0
  5b7f9a:	41 23 c7                                        	and    eax,r15d
  5b7f9d:	7d 07                                           	jge    0x5b7fa6
  5b7f9f:	ff c8                                           	dec    eax
  5b7fa1:	41 0b c6                                        	or     eax,r14d
  5b7fa4:	ff c0                                           	inc    eax
  5b7fa6:	88 44 1d 97                                     	mov    BYTE PTR [rbp+rbx*1-0x69],al
  5b7faa:	e8 41 f9 61 00                                  	call   0xbd78f0
  5b7faf:	41 23 c7                                        	and    eax,r15d
  5b7fb2:	7d 07                                           	jge    0x5b7fbb
  5b7fb4:	ff c8                                           	dec    eax
  5b7fb6:	41 0b c6                                        	or     eax,r14d
  5b7fb9:	ff c0                                           	inc    eax
  5b7fbb:	88 44 1d a7                                     	mov    BYTE PTR [rbp+rbx*1-0x59],al
  5b7fbf:	e8 2c f9 61 00                                  	call   0xbd78f0
  5b7fc4:	41 23 c7                                        	and    eax,r15d
  5b7fc7:	7d 07                                           	jge    0x5b7fd0
  5b7fc9:	ff c8                                           	dec    eax
  5b7fcb:	41 0b c6                                        	or     eax,r14d
  5b7fce:	ff c0                                           	inc    eax
  5b7fd0:	88 44 1d b7                                     	mov    BYTE PTR [rbp+rbx*1-0x49],al
  5b7fd4:	e8 17 f9 61 00                                  	call   0xbd78f0
  5b7fd9:	41 23 c7                                        	and    eax,r15d
  5b7fdc:	7d 07                                           	jge    0x5b7fe5
  5b7fde:	ff c8                                           	dec    eax
  5b7fe0:	41 0b c6                                        	or     eax,r14d
  5b7fe3:	ff c0                                           	inc    eax
  5b7fe5:	88 44 1d c7                                     	mov    BYTE PTR [rbp+rbx*1-0x39],al
  5b7fe9:	48 ff c3                                        	inc    rbx
  5b7fec:	48 83 fb 10                                     	cmp    rbx,0x10
  5b7ff0:	7c a3                                           	jl     0x5b7f95
  5b7ff2:	4c 8d a7 58 01 00 00                            	lea    r12,[rdi+0x158]
  5b7ff9:	4d 85 e4                                        	test   r12,r12
  5b7ffc:	0f 84 a3 04 00 00                               	je     0x5b84a5
  5b8002:	e8 91 8a b7 ff                                  	call   0x130a98
  5b8007:	48 8b 96 e8 00 00 00                            	mov    rdx,QWORD PTR [rsi+0xe8]
  5b800e:	4c 8d 45 87                                     	lea    r8,[rbp-0x79]
  5b8012:	4c 8b f0                                        	mov    r14,rax
  5b8015:	c7 45 87 35 00 00 00                            	mov    DWORD PTR [rbp-0x79],0x35
  5b801c:	4c 89 6d 8f                                     	mov    QWORD PTR [rbp-0x71],r13
  5b8020:	48 8b 08                                        	mov    rcx,QWORD PTR [rax]
  5b8023:	4c 8b 49 28                                     	mov    r9,QWORD PTR [rcx+0x28]
  5b8027:	48 8b c8                                        	mov    rcx,rax
  5b802a:	41 ff d1                                        	call   r9
  5b802d:	0f 28 75 a7                                     	movaps xmm6,XMMWORD PTR [rbp-0x59]
  5b8031:	4c 8b f8                                        	mov    r15,rax
  5b8034:	48 85 c0                                        	test   rax,rax
  5b8037:	0f 84 c1 00 00 00                               	je     0x5b80fe
  5b803d:	48 8d 55 97                                     	lea    rdx,[rbp-0x69]
  5b8041:	48 8d 0d 48 a6 5a 04                            	lea    rcx,[rip+0x45aa648]        # 0x4b62690
  5b8048:	e8 af 07 00 00                                  	call   0x5b87fc
  5b804d:	8b 96 e8 00 00 00                               	mov    edx,DWORD PTR [rsi+0xe8]
  5b8053:	48 8d 44 24 40                                  	lea    rax,[rsp+0x40]
  5b8058:	4c 8b 86 e0 00 00 00                            	mov    r8,QWORD PTR [rsi+0xe0]
  5b805f:	44 8b ca                                        	mov    r9d,edx
  5b8062:	49 8b cf                                        	mov    rcx,r15
  5b8065:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
  5b806a:	66 0f 7f 74 24 40                               	movdqa XMMWORD PTR [rsp+0x40],xmm6
  5b8070:	e8 a3 04 00 00                                  	call   0x5b8518
  5b8075:	4c 8b 86 e8 00 00 00                            	mov    r8,QWORD PTR [rsi+0xe8]
  5b807c:	85 c0                                           	test   eax,eax
  5b807e:	48 8b 8e e0 00 00 00                            	mov    rcx,QWORD PTR [rsi+0xe0]
  5b8085:	b8 0d 00 00 00                                  	mov    eax,0xd
  5b808a:	41 8b dd                                        	mov    ebx,r13d
  5b808d:	49 8b d7                                        	mov    rdx,r15
  5b8090:	0f 44 d8                                        	cmove  ebx,eax
  5b8093:	e8 88 c0 5d 00                                  	call   0xb94120
  5b8098:	48 8d 55 b7                                     	lea    rdx,[rbp-0x49]
  5b809c:	48 8d 0d ed a5 5a 04                            	lea    rcx,[rip+0x45aa5ed]        # 0x4b62690
  5b80a3:	e8 54 07 00 00                                  	call   0x5b87fc
  5b80a8:	0f 28 45 c7                                     	movaps xmm0,XMMWORD PTR [rbp-0x39]
  5b80ac:	48 8d 44 24 40                                  	lea    rax,[rsp+0x40]
  5b80b1:	8b 96 e8 00 00 00                               	mov    edx,DWORD PTR [rsi+0xe8]
  5b80b7:	49 8b cf                                        	mov    rcx,r15
  5b80ba:	4c 8b 86 e0 00 00 00                            	mov    r8,QWORD PTR [rsi+0xe0]
  5b80c1:	44 8b ca                                        	mov    r9d,edx
  5b80c4:	66 0f 7f 44 24 40                               	movdqa XMMWORD PTR [rsp+0x40],xmm0
  5b80ca:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
  5b80cf:	e8 44 04 00 00                                  	call   0x5b8518
  5b80d4:	4c 8b 86 e8 00 00 00                            	mov    r8,QWORD PTR [rsi+0xe8]
  5b80db:	85 c0                                           	test   eax,eax
  5b80dd:	b8 0d 00 00 00                                  	mov    eax,0xd
  5b80e2:	49 8b d7                                        	mov    rdx,r15
  5b80e5:	49 8b cc                                        	mov    rcx,r12
  5b80e8:	0f 44 d8                                        	cmove  ebx,eax
  5b80eb:	e8 30 c0 5d 00                                  	call   0xb94120
  5b80f0:	49 8b 06                                        	mov    rax,QWORD PTR [r14]
  5b80f3:	49 8b d7                                        	mov    rdx,r15
  5b80f6:	49 8b ce                                        	mov    rcx,r14
  5b80f9:	ff 50 58                                        	call   QWORD PTR [rax+0x58]
  5b80fc:	eb 05                                           	jmp    0x5b8103
  5b80fe:	bb 04 00 00 00                                  	mov    ebx,0x4
  5b8103:	85 db                                           	test   ebx,ebx
  5b8105:	0f 85 9f 03 00 00                               	jne    0x5b84aa
  5b810b:	e8 88 89 b7 ff                                  	call   0x130a98
  5b8110:	4c 8d 44 24 30                                  	lea    r8,[rsp+0x30]
  5b8115:	c7 44 24 30 35 00 00 00                         	mov    DWORD PTR [rsp+0x30],0x35
  5b811d:	ba 58 01 00 00                                  	mov    edx,0x158
  5b8122:	4c 89 6c 24 38                                  	mov    QWORD PTR [rsp+0x38],r13
  5b8127:	4c 8b f0                                        	mov    r14,rax
  5b812a:	48 8b 08                                        	mov    rcx,QWORD PTR [rax]
  5b812d:	4c 8b 49 28                                     	mov    r9,QWORD PTR [rcx+0x28]
  5b8131:	48 8b c8                                        	mov    rcx,rax
  5b8134:	41 ff d1                                        	call   r9
  5b8137:	4c 8b f8                                        	mov    r15,rax
  5b813a:	48 85 c0                                        	test   rax,rax
  5b813d:	0f 84 62 03 00 00                               	je     0x5b84a5
  5b8143:	48 8d 55 97                                     	lea    rdx,[rbp-0x69]
  5b8147:	48 8d 0d 42 a5 5a 04                            	lea    rcx,[rip+0x45aa542]        # 0x4b62690
  5b814e:	e8 a9 06 00 00                                  	call   0x5b87fc
  5b8153:	44 8d 63 10                                     	lea    r12d,[rbx+0x10]
  5b8157:	66 0f 7f 74 24 40                               	movdqa XMMWORD PTR [rsp+0x40],xmm6
  5b815d:	48 8d 44 24 40                                  	lea    rax,[rsp+0x40]
  5b8162:	45 8b cc                                        	mov    r9d,r12d
  5b8165:	41 8b d4                                        	mov    edx,r12d
  5b8168:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
  5b816d:	48 8d 4f 69                                     	lea    rcx,[rdi+0x69]
  5b8171:	4c 8d 45 b7                                     	lea    r8,[rbp-0x49]
  5b8175:	e8 9e 03 00 00                                  	call   0x5b8518
  5b817a:	85 c0                                           	test   eax,eax
  5b817c:	66 0f 7f 74 24 40                               	movdqa XMMWORD PTR [rsp+0x40],xmm6
  5b8182:	8d 43 0d                                        	lea    eax,[rbx+0xd]
  5b8185:	45 8b cc                                        	mov    r9d,r12d
  5b8188:	0f 44 d8                                        	cmove  ebx,eax
  5b818b:	48 8d 4f 79                                     	lea    rcx,[rdi+0x79]
  5b818f:	48 8d 44 24 40                                  	lea    rax,[rsp+0x40]
  5b8194:	41 8b d4                                        	mov    edx,r12d
  5b8197:	4c 8d 45 c7                                     	lea    r8,[rbp-0x39]
  5b819b:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
  5b81a0:	e8 73 03 00 00                                  	call   0x5b8518
  5b81a5:	85 c0                                           	test   eax,eax
  5b81a7:	4c 8d 1d 52 7e a4 ff                            	lea    r11,[rip+0xffffffffffa47e52]        # 0x0
  5b81ae:	41 8d 44 24 fd                                  	lea    eax,[r12-0x3]
  5b81b3:	4d 8b d5                                        	mov    r10,r13
  5b81b6:	0f 44 d8                                        	cmove  ebx,eax
  5b81b9:	47 8a 8c 1a c8 4b bd 03                         	mov    r9b,BYTE PTR [r10+r11*1+0x3bd4bc8]
  5b81c1:	47 8a 84 1a d0 4b bd 03                         	mov    r8b,BYTE PTR [r10+r11*1+0x3bd4bd0]
  5b81c9:	41 8a c1                                        	mov    al,r9b
  5b81cc:	43 32 84 1a b0 78 bd 03                         	xor    al,BYTE PTR [r10+r11*1+0x3bd78b0]
  5b81d4:	43 8a 94 1a cc 4b bd 03                         	mov    dl,BYTE PTR [r10+r11*1+0x3bd4bcc]
  5b81dc:	43 8a 8c 1a d4 4b bd 03                         	mov    cl,BYTE PTR [r10+r11*1+0x3bd4bd4]
  5b81e4:	47 32 8c 1a c0 78 bd 03                         	xor    r9b,BYTE PTR [r10+r11*1+0x3bd78c0]
  5b81ec:	42 88 44 14 40                                  	mov    BYTE PTR [rsp+r10*1+0x40],al
  5b81f1:	41 8a c0                                        	mov    al,r8b
  5b81f4:	43 32 84 1a b4 78 bd 03                         	xor    al,BYTE PTR [r10+r11*1+0x3bd78b4]
  5b81fc:	47 32 84 1a c4 78 bd 03                         	xor    r8b,BYTE PTR [r10+r11*1+0x3bd78c4]
  5b8204:	42 88 44 14 44                                  	mov    BYTE PTR [rsp+r10*1+0x44],al
  5b8209:	8a c2                                           	mov    al,dl
  5b820b:	43 32 84 1a b8 78 bd 03                         	xor    al,BYTE PTR [r10+r11*1+0x3bd78b8]
  5b8213:	43 32 94 1a c8 78 bd 03                         	xor    dl,BYTE PTR [r10+r11*1+0x3bd78c8]
  5b821b:	42 88 44 14 48                                  	mov    BYTE PTR [rsp+r10*1+0x48],al
  5b8220:	8a c1                                           	mov    al,cl
  5b8222:	43 32 84 1a bc 78 bd 03                         	xor    al,BYTE PTR [r10+r11*1+0x3bd78bc]
  5b822a:	43 32 8c 1a cc 78 bd 03                         	xor    cl,BYTE PTR [r10+r11*1+0x3bd78cc]
  5b8232:	42 88 44 15 83                                  	mov    BYTE PTR [rbp+r10*1-0x7d],al
  5b8237:	46 88 4c 15 87                                  	mov    BYTE PTR [rbp+r10*1-0x79],r9b
  5b823c:	46 88 44 15 8b                                  	mov    BYTE PTR [rbp+r10*1-0x75],r8b
  5b8241:	42 88 54 15 8f                                  	mov    BYTE PTR [rbp+r10*1-0x71],dl
  5b8246:	42 88 4c 15 93                                  	mov    BYTE PTR [rbp+r10*1-0x6d],cl
  5b824b:	49 ff c2                                        	inc    r10
  5b824e:	49 83 fa 04                                     	cmp    r10,0x4
  5b8252:	0f 8c 61 ff ff ff                               	jl     0x5b81b9
  5b8258:	48 8d 54 24 40                                  	lea    rdx,[rsp+0x40]
  5b825d:	48 8d 0d 2c a4 5a 04                            	lea    rcx,[rip+0x45aa42c]        # 0x4b62690
  5b8264:	e8 93 05 00 00                                  	call   0x5b87fc
  5b8269:	0f 28 75 87                                     	movaps xmm6,XMMWORD PTR [rbp-0x79]
  5b826d:	48 8d 44 24 40                                  	lea    rax,[rsp+0x40]
  5b8272:	48 8d 4f 49                                     	lea    rcx,[rdi+0x49]
  5b8276:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
  5b827b:	45 8b cc                                        	mov    r9d,r12d
  5b827e:	66 0f 7f 74 24 40                               	movdqa XMMWORD PTR [rsp+0x40],xmm6
  5b8284:	4c 8d 45 97                                     	lea    r8,[rbp-0x69]
  5b8288:	41 8b d4                                        	mov    edx,r12d
  5b828b:	e8 88 02 00 00                                  	call   0x5b8518
  5b8290:	85 c0                                           	test   eax,eax
  5b8292:	66 0f 7f 74 24 40                               	movdqa XMMWORD PTR [rsp+0x40],xmm6
  5b8298:	b8 0d 00 00 00                                  	mov    eax,0xd
  5b829d:	48 8d 4f 59                                     	lea    rcx,[rdi+0x59]
  5b82a1:	0f 44 d8                                        	cmove  ebx,eax
  5b82a4:	4c 8d 45 a7                                     	lea    r8,[rbp-0x59]
  5b82a8:	48 8d 44 24 40                                  	lea    rax,[rsp+0x40]
  5b82ad:	45 8b cc                                        	mov    r9d,r12d
  5b82b0:	41 8b d4                                        	mov    edx,r12d
  5b82b3:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
  5b82b8:	e8 5b 02 00 00                                  	call   0x5b8518
  5b82bd:	85 c0                                           	test   eax,eax
  5b82bf:	4c 8d 4d f7                                     	lea    r9,[rbp-0x9]
  5b82c3:	41 bc 0d 00 00 00                               	mov    r12d,0xd
  5b82c9:	4c 8d 45 e7                                     	lea    r8,[rbp-0x19]
  5b82cd:	48 8d 55 d7                                     	lea    rdx,[rbp-0x29]
  5b82d1:	41 0f 44 dc                                     	cmove  ebx,r12d
  5b82d5:	48 8d 4d 87                                     	lea    rcx,[rbp-0x79]
  5b82d9:	e8 62 0c a0 00                                  	call   0xfb8f40
  5b82de:	84 c0                                           	test   al,al
  5b82e0:	48 8d 55 87                                     	lea    rdx,[rbp-0x79]
  5b82e4:	48 8d 0d a5 a3 5a 04                            	lea    rcx,[rip+0x45aa3a5]        # 0x4b62690
  5b82eb:	41 0f 44 dc                                     	cmove  ebx,r12d
  5b82ef:	e8 08 05 00 00                                  	call   0x5b87fc
  5b82f4:	0f 28 45 d7                                     	movaps xmm0,XMMWORD PTR [rbp-0x29]
  5b82f8:	48 8d 44 24 40                                  	lea    rax,[rsp+0x40]
  5b82fd:	41 b9 58 01 00 00                               	mov    r9d,0x158
  5b8303:	66 0f 7f 44 24 40                               	movdqa XMMWORD PTR [rsp+0x40],xmm0
  5b8309:	41 8b d1                                        	mov    edx,r9d
  5b830c:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
  5b8311:	4c 8b c7                                        	mov    r8,rdi
  5b8314:	49 8b cf                                        	mov    rcx,r15
  5b8317:	e8 fc 01 00 00                                  	call   0x5b8518
  5b831c:	85 c0                                           	test   eax,eax
  5b831e:	48 8b d7                                        	mov    rdx,rdi
  5b8321:	49 8b c7                                        	mov    rax,r15
  5b8324:	41 0f 44 dc                                     	cmove  ebx,r12d
  5b8328:	41 bc 02 00 00 00                               	mov    r12d,0x2
  5b832e:	41 8b cc                                        	mov    ecx,r12d
  5b8331:	45 8d 44 24 7e                                  	lea    r8d,[r12+0x7e]
  5b8336:	0f 10 00                                        	movups xmm0,XMMWORD PTR [rax]
  5b8339:	0f 11 02                                        	movups XMMWORD PTR [rdx],xmm0
  5b833c:	0f 10 48 10                                     	movups xmm1,XMMWORD PTR [rax+0x10]
  5b8340:	0f 11 4a 10                                     	movups XMMWORD PTR [rdx+0x10],xmm1
  5b8344:	0f 10 40 20                                     	movups xmm0,XMMWORD PTR [rax+0x20]
  5b8348:	0f 11 42 20                                     	movups XMMWORD PTR [rdx+0x20],xmm0
  5b834c:	0f 10 48 30                                     	movups xmm1,XMMWORD PTR [rax+0x30]
  5b8350:	0f 11 4a 30                                     	movups XMMWORD PTR [rdx+0x30],xmm1
  5b8354:	0f 10 40 40                                     	movups xmm0,XMMWORD PTR [rax+0x40]
  5b8358:	0f 11 42 40                                     	movups XMMWORD PTR [rdx+0x40],xmm0
  5b835c:	0f 10 48 50                                     	movups xmm1,XMMWORD PTR [rax+0x50]
  5b8360:	0f 11 4a 50                                     	movups XMMWORD PTR [rdx+0x50],xmm1
  5b8364:	0f 10 40 60                                     	movups xmm0,XMMWORD PTR [rax+0x60]
  5b8368:	0f 11 42 60                                     	movups XMMWORD PTR [rdx+0x60],xmm0
  5b836c:	49 03 d0                                        	add    rdx,r8
  5b836f:	0f 10 48 70                                     	movups xmm1,XMMWORD PTR [rax+0x70]
  5b8373:	49 03 c0                                        	add    rax,r8
  5b8376:	0f 11 4a f0                                     	movups XMMWORD PTR [rdx-0x10],xmm1
  5b837a:	48 83 e9 01                                     	sub    rcx,0x1
  5b837e:	75 b6                                           	jne    0x5b8336
  5b8380:	0f 10 00                                        	movups xmm0,XMMWORD PTR [rax]
  5b8383:	48 8d 0d 06 a3 5a 04                            	lea    rcx,[rip+0x45aa306]        # 0x4b62690
  5b838a:	0f 11 02                                        	movups XMMWORD PTR [rdx],xmm0
  5b838d:	0f 10 48 10                                     	movups xmm1,XMMWORD PTR [rax+0x10]
  5b8391:	0f 11 4a 10                                     	movups XMMWORD PTR [rdx+0x10],xmm1
  5b8395:	0f 10 40 20                                     	movups xmm0,XMMWORD PTR [rax+0x20]
  5b8399:	0f 11 42 20                                     	movups XMMWORD PTR [rdx+0x20],xmm0
  5b839d:	0f 10 48 30                                     	movups xmm1,XMMWORD PTR [rax+0x30]
  5b83a1:	0f 11 4a 30                                     	movups XMMWORD PTR [rdx+0x30],xmm1
  5b83a5:	0f 10 40 40                                     	movups xmm0,XMMWORD PTR [rax+0x40]
  5b83a9:	0f 11 42 40                                     	movups XMMWORD PTR [rdx+0x40],xmm0
  5b83ad:	48 8b 40 50                                     	mov    rax,QWORD PTR [rax+0x50]
  5b83b1:	48 89 42 50                                     	mov    QWORD PTR [rdx+0x50],rax
  5b83b5:	48 8d 55 e7                                     	lea    rdx,[rbp-0x19]
  5b83b9:	e8 3e 04 00 00                                  	call   0x5b87fc
  5b83be:	0f 28 45 f7                                     	movaps xmm0,XMMWORD PTR [rbp-0x9]
  5b83c2:	48 8d 44 24 40                                  	lea    rax,[rsp+0x40]
  5b83c7:	41 b9 58 01 00 00                               	mov    r9d,0x158
  5b83cd:	66 0f 7f 44 24 40                               	movdqa XMMWORD PTR [rsp+0x40],xmm0
  5b83d3:	41 8b d1                                        	mov    edx,r9d
  5b83d6:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
  5b83db:	4c 8b c7                                        	mov    r8,rdi
  5b83de:	49 8b cf                                        	mov    rcx,r15
  5b83e1:	e8 32 01 00 00                                  	call   0x5b8518
  5b83e6:	85 c0                                           	test   eax,eax
  5b83e8:	48 8b cf                                        	mov    rcx,rdi
  5b83eb:	b8 0d 00 00 00                                  	mov    eax,0xd
  5b83f0:	ba 80 00 00 00                                  	mov    edx,0x80
  5b83f5:	0f 44 d8                                        	cmove  ebx,eax
  5b83f8:	49 8b c7                                        	mov    rax,r15
  5b83fb:	0f 10 00                                        	movups xmm0,XMMWORD PTR [rax]
  5b83fe:	0f 11 01                                        	movups XMMWORD PTR [rcx],xmm0
  5b8401:	0f 10 48 10                                     	movups xmm1,XMMWORD PTR [rax+0x10]
  5b8405:	0f 11 49 10                                     	movups XMMWORD PTR [rcx+0x10],xmm1
  5b8409:	0f 10 40 20                                     	movups xmm0,XMMWORD PTR [rax+0x20]
  5b840d:	0f 11 41 20                                     	movups XMMWORD PTR [rcx+0x20],xmm0
  5b8411:	0f 10 48 30                                     	movups xmm1,XMMWORD PTR [rax+0x30]
  5b8415:	0f 11 49 30                                     	movups XMMWORD PTR [rcx+0x30],xmm1
  5b8419:	0f 10 40 40                                     	movups xmm0,XMMWORD PTR [rax+0x40]
  5b841d:	0f 11 41 40                                     	movups XMMWORD PTR [rcx+0x40],xmm0
  5b8421:	0f 10 48 50                                     	movups xmm1,XMMWORD PTR [rax+0x50]
  5b8425:	0f 11 49 50                                     	movups XMMWORD PTR [rcx+0x50],xmm1
  5b8429:	0f 10 40 60                                     	movups xmm0,XMMWORD PTR [rax+0x60]
  5b842d:	0f 11 41 60                                     	movups XMMWORD PTR [rcx+0x60],xmm0
  5b8431:	48 03 ca                                        	add    rcx,rdx
  5b8434:	0f 10 48 70                                     	movups xmm1,XMMWORD PTR [rax+0x70]
  5b8438:	48 03 c2                                        	add    rax,rdx
  5b843b:	0f 11 49 f0                                     	movups XMMWORD PTR [rcx-0x10],xmm1
  5b843f:	49 83 ec 01                                     	sub    r12,0x1
  5b8443:	75 b6                                           	jne    0x5b83fb
  5b8445:	0f 10 00                                        	movups xmm0,XMMWORD PTR [rax]
  5b8448:	0f 11 01                                        	movups XMMWORD PTR [rcx],xmm0
  5b844b:	0f 10 48 10                                     	movups xmm1,XMMWORD PTR [rax+0x10]
  5b844f:	0f 11 49 10                                     	movups XMMWORD PTR [rcx+0x10],xmm1
  5b8453:	0f 10 40 20                                     	movups xmm0,XMMWORD PTR [rax+0x20]
  5b8457:	0f 11 41 20                                     	movups XMMWORD PTR [rcx+0x20],xmm0
  5b845b:	0f 10 48 30                                     	movups xmm1,XMMWORD PTR [rax+0x30]
  5b845f:	0f 11 49 30                                     	movups XMMWORD PTR [rcx+0x30],xmm1
  5b8463:	0f 10 40 40                                     	movups xmm0,XMMWORD PTR [rax+0x40]
  5b8467:	0f 11 41 40                                     	movups XMMWORD PTR [rcx+0x40],xmm0
  5b846b:	48 8b 40 50                                     	mov    rax,QWORD PTR [rax+0x50]
  5b846f:	48 89 41 50                                     	mov    QWORD PTR [rcx+0x50],rax
  5b8473:	48 8d 4d 87                                     	lea    rcx,[rbp-0x79]
  5b8477:	e8 34 16 a0 00                                  	call   0xfb9ab0
  5b847c:	48 8d 4d d7                                     	lea    rcx,[rbp-0x29]
  5b8480:	e8 2b 16 a0 00                                  	call   0xfb9ab0
  5b8485:	48 8d 4d e7                                     	lea    rcx,[rbp-0x19]
  5b8489:	e8 22 16 a0 00                                  	call   0xfb9ab0
  5b848e:	48 8d 4d f7                                     	lea    rcx,[rbp-0x9]
  5b8492:	e8 19 16 a0 00                                  	call   0xfb9ab0
  5b8497:	49 8b 06                                        	mov    rax,QWORD PTR [r14]
  5b849a:	49 8b d7                                        	mov    rdx,r15
  5b849d:	49 8b ce                                        	mov    rcx,r14
  5b84a0:	ff 50 58                                        	call   QWORD PTR [rax+0x58]
  5b84a3:	eb 05                                           	jmp    0x5b84aa
  5b84a5:	bb 04 00 00 00                                  	mov    ebx,0x4
  5b84aa:	48 8d 4d 97                                     	lea    rcx,[rbp-0x69]
  5b84ae:	e8 fd 15 a0 00                                  	call   0xfb9ab0
  5b84b3:	48 8d 4d a7                                     	lea    rcx,[rbp-0x59]
  5b84b7:	e8 f4 15 a0 00                                  	call   0xfb9ab0
  5b84bc:	48 8d 4d b7                                     	lea    rcx,[rbp-0x49]
  5b84c0:	e8 eb 15 a0 00                                  	call   0xfb9ab0
  5b84c5:	48 8d 4d c7                                     	lea    rcx,[rbp-0x39]
  5b84c9:	e8 e2 15 a0 00                                  	call   0xfb9ab0
  5b84ce:	85 db                                           	test   ebx,ebx
  5b84d0:	74 18                                           	je     0x5b84ea
  5b84d2:	4c 8b 86 e8 00 00 00                            	mov    r8,QWORD PTR [rsi+0xe8]
  5b84d9:	33 d2                                           	xor    edx,edx
  5b84db:	49 81 c0 58 01 00 00                            	add    r8,0x158
  5b84e2:	48 8b cf                                        	mov    rcx,rdi
  5b84e5:	e8 e6 b8 5d 00                                  	call   0xb93dd0
  5b84ea:	8b c3                                           	mov    eax,ebx
  5b84ec:	48 8b 4d 07                                     	mov    rcx,QWORD PTR [rbp+0x7]
  5b84f0:	48 33 cc                                        	xor    rcx,rsp
  5b84f3:	e8 b8 8c 5d 00                                  	call   0xb911b0
  5b84f8:	4c 8d 9c 24 f0 00 00 00                         	lea    r11,[rsp+0xf0]
  5b8500:	49 8b 5b 50                                     	mov    rbx,QWORD PTR [r11+0x50]
  5b8504:	41 0f 28 73 f0                                  	movaps xmm6,XMMWORD PTR [r11-0x10]
  5b8509:	49 8b e3                                        	mov    rsp,r11
  5b850c:	41 5f                                           	pop    r15
  5b850e:	41 5e                                           	pop    r14
  5b8510:	41 5d                                           	pop    r13
  5b8512:	41 5c                                           	pop    r12
  5b8514:	5f                                              	pop    rdi
  5b8515:	5e                                              	pop    rsi
  5b8516:	5d                                              	pop    rbp
  5b8517:	c3                                              	ret
