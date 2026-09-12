
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b6f64 <.data>:
  5b6f64:	48 89 5c 24 08                                  	mov    QWORD PTR [rsp+0x8],rbx
  5b6f69:	48 89 74 24 10                                  	mov    QWORD PTR [rsp+0x10],rsi
  5b6f6e:	48 89 7c 24 18                                  	mov    QWORD PTR [rsp+0x18],rdi
  5b6f73:	55                                              	push   rbp
  5b6f74:	41 56                                           	push   r14
  5b6f76:	41 57                                           	push   r15
  5b6f78:	48 8b ec                                        	mov    rbp,rsp
  5b6f7b:	48 81 ec 80 00 00 00                            	sub    rsp,0x80
  5b6f82:	33 ff                                           	xor    edi,edi
  5b6f84:	45 8a f1                                        	mov    r14b,r9b
  5b6f87:	45 8b f8                                        	mov    r15d,r8d
  5b6f8a:	8b f2                                           	mov    esi,edx
  5b6f8c:	48 8b d9                                        	mov    rbx,rcx
  5b6f8f:	40 38 79 39                                     	cmp    BYTE PTR [rcx+0x39],dil
  5b6f93:	0f 85 fd 59 42 01                               	jne    0x19dc996
  5b6f99:	85 d2                                           	test   edx,edx
  5b6f9b:	74 05                                           	je     0x5b6fa2
  5b6f9d:	83 fa 02                                        	cmp    edx,0x2
  5b6fa0:	75 05                                           	jne    0x5b6fa7
  5b6fa2:	e8 71 05 00 00                                  	call   0x5b7518
  5b6fa7:	45 84 f6                                        	test   r14b,r14b
  5b6faa:	74 04                                           	je     0x5b6fb0
  5b6fac:	44 8b 7b 28                                     	mov    r15d,DWORD PTR [rbx+0x28]
  5b6fb0:	8b ce                                           	mov    ecx,esi
  5b6fb2:	85 f6                                           	test   esi,esi
  5b6fb4:	74 0e                                           	je     0x5b6fc4
  5b6fb6:	83 e9 01                                        	sub    ecx,0x1
  5b6fb9:	74 09                                           	je     0x5b6fc4
  5b6fbb:	83 e9 01                                        	sub    ecx,0x1
  5b6fbe:	0f 85 e6 59 42 01                               	jne    0x19dc9aa
  5b6fc4:	48 8d 4d a0                                     	lea    rcx,[rbp-0x60]
  5b6fc8:	e8 13 05 00 00                                  	call   0x5b74e0
  5b6fcd:	8a 4b 39                                        	mov    cl,BYTE PTR [rbx+0x39]
  5b6fd0:	89 75 a0                                        	mov    DWORD PTR [rbp-0x60],esi
  5b6fd3:	84 c9                                           	test   cl,cl
  5b6fd5:	0f 85 ef 59 42 01                               	jne    0x19dc9ca
  5b6fdb:	39 7b 34                                        	cmp    DWORD PTR [rbx+0x34],edi
  5b6fde:	0f 85 e6 59 42 01                               	jne    0x19dc9ca
  5b6fe4:	48 8b 03                                        	mov    rax,QWORD PTR [rbx]
  5b6fe7:	48 c7 45 b0 58 00 90 00                         	mov    QWORD PTR [rbp-0x50],0x900058
  5b6fef:	0f 10 43 48                                     	movups xmm0,XMMWORD PTR [rbx+0x48]
  5b6ff3:	48 89 45 a8                                     	mov    QWORD PTR [rbp-0x58],rax
  5b6ff7:	48 8d 05 92 b3 32 03                            	lea    rax,[rip+0x332b392]        # 0x38e2390
  5b6ffe:	0f 10 4b 58                                     	movups xmm1,XMMWORD PTR [rbx+0x58]
  5b7002:	88 4d c4                                        	mov    BYTE PTR [rbp-0x3c],cl
  5b7005:	48 8d 4d a0                                     	lea    rcx,[rbp-0x60]
  5b7009:	0f 11 45 c8                                     	movups XMMWORD PTR [rbp-0x38],xmm0
  5b700d:	c7 45 a4 00 00 01 02                            	mov    DWORD PTR [rbp-0x5c],0x2010000
  5b7014:	f2 0f 10 43 68                                  	movsd  xmm0,QWORD PTR [rbx+0x68]
  5b7019:	f2 0f 11 45 e8                                  	movsd  QWORD PTR [rbp-0x18],xmm0
  5b701e:	48 89 45 b8                                     	mov    QWORD PTR [rbp-0x48],rax
  5b7022:	44 89 7d c0                                     	mov    DWORD PTR [rbp-0x40],r15d
  5b7026:	0f 11 4d d8                                     	movups XMMWORD PTR [rbp-0x28],xmm1
  5b702a:	89 7d f0                                        	mov    DWORD PTR [rbp-0x10],edi
  5b702d:	e8 1e 00 00 00                                  	call   0x5b7050
  5b7032:	4c 8d 9c 24 80 00 00 00                         	lea    r11,[rsp+0x80]
  5b703a:	49 8b 5b 20                                     	mov    rbx,QWORD PTR [r11+0x20]
  5b703e:	49 8b 73 28                                     	mov    rsi,QWORD PTR [r11+0x28]
  5b7042:	49 8b 7b 30                                     	mov    rdi,QWORD PTR [r11+0x30]
  5b7046:	49 8b e3                                        	mov    rsp,r11
  5b7049:	41 5f                                           	pop    r15
  5b704b:	41 5e                                           	pop    r14
  5b704d:	5d                                              	pop    rbp
  5b704e:	c3                                              	ret
