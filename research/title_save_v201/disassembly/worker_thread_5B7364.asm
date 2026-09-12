
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b7364 <.data>:
  5b7364:	48 8b c4                                        	mov    rax,rsp
  5b7367:	48 89 58 10                                     	mov    QWORD PTR [rax+0x10],rbx
  5b736b:	48 89 68 18                                     	mov    QWORD PTR [rax+0x18],rbp
  5b736f:	48 89 70 20                                     	mov    QWORD PTR [rax+0x20],rsi
  5b7373:	57                                              	push   rdi
  5b7374:	41 56                                           	push   r14
  5b7376:	41 57                                           	push   r15
  5b7378:	48 83 ec 40                                     	sub    rsp,0x40
  5b737c:	48 8b e9                                        	mov    rbp,rcx
  5b737f:	c7 40 d8 02 00 00 00                            	mov    DWORD PTR [rax-0x28],0x2
  5b7386:	48 8b 49 20                                     	mov    rcx,QWORD PTR [rcx+0x20]
  5b738a:	45 33 ff                                        	xor    r15d,r15d
  5b738d:	4c 89 78 e0                                     	mov    QWORD PTR [rax-0x20],r15
  5b7391:	49 8b f8                                        	mov    rdi,r8
  5b7394:	48 8b f2                                        	mov    rsi,rdx
  5b7397:	4c 8d 44 24 30                                  	lea    r8,[rsp+0x30]
  5b739c:	4d 8b f1                                        	mov    r14,r9
  5b739f:	48 8b 01                                        	mov    rax,QWORD PTR [rcx]
  5b73a2:	41 8d 57 38                                     	lea    edx,[r15+0x38]
  5b73a6:	ff 50 30                                        	call   QWORD PTR [rax+0x30]
  5b73a9:	48 8b d8                                        	mov    rbx,rax
  5b73ac:	48 85 c0                                        	test   rax,rax
  5b73af:	75 07                                           	jne    0x5b73b8
  5b73b1:	33 c0                                           	xor    eax,eax
  5b73b3:	e9 e5 00 00 00                                  	jmp    0x5b749d
  5b73b8:	45 33 c9                                        	xor    r9d,r9d
  5b73bb:	48 89 78 28                                     	mov    QWORD PTR [rax+0x28],rdi
  5b73bf:	45 33 c0                                        	xor    r8d,r8d
  5b73c2:	48 89 70 30                                     	mov    QWORD PTR [rax+0x30],rsi
  5b73c6:	33 d2                                           	xor    edx,edx
  5b73c8:	33 c9                                           	xor    ecx,ecx
  5b73ca:	ff 15 f8 71 32 03                               	call   QWORD PTR [rip+0x33271f8]        # 0x38de5c8
  5b73d0:	48 8b f0                                        	mov    rsi,rax
  5b73d3:	48 85 c0                                        	test   rax,rax
  5b73d6:	75 0f                                           	jne    0x5b73e7
  5b73d8:	48 8b 4d 20                                     	mov    rcx,QWORD PTR [rbp+0x20]
  5b73dc:	48 8b d3                                        	mov    rdx,rbx
  5b73df:	48 8b 01                                        	mov    rax,QWORD PTR [rcx]
  5b73e2:	ff 50 58                                        	call   QWORD PTR [rax+0x58]
  5b73e5:	eb ca                                           	jmp    0x5b73b1
  5b73e7:	48 8d 44 24 60                                  	lea    rax,[rsp+0x60]
  5b73ec:	4c 8b cb                                        	mov    r9,rbx
  5b73ef:	48 89 44 24 28                                  	mov    QWORD PTR [rsp+0x28],rax
  5b73f4:	4c 8d 05 a5 f1 ff ff                            	lea    r8,[rip+0xfffffffffffff1a5]        # 0x5b65a0
  5b73fb:	41 8b d6                                        	mov    edx,r14d
  5b73fe:	c7 44 24 20 04 00 00 00                         	mov    DWORD PTR [rsp+0x20],0x4
  5b7406:	33 c9                                           	xor    ecx,ecx
  5b7408:	e8 a7 45 62 00                                  	call   0xbdb9b4
  5b740d:	48 8b f8                                        	mov    rdi,rax
  5b7410:	48 85 c0                                        	test   rax,rax
  5b7413:	75 0b                                           	jne    0x5b7420
  5b7415:	48 8b ce                                        	mov    rcx,rsi
  5b7418:	ff 15 72 6f 32 03                               	call   QWORD PTR [rip+0x3326f72]        # 0x38de390
  5b741e:	eb b8                                           	jmp    0x5b73d8
  5b7420:	48 63 84 24 80 00 00 00                         	movsxd rax,DWORD PTR [rsp+0x80]
  5b7428:	48 8d 0d a9 6a 64 03                            	lea    rcx,[rip+0x3646aa9]        # 0x3bfded8
  5b742f:	8b 14 81                                        	mov    edx,DWORD PTR [rcx+rax*4]
  5b7432:	48 8b cf                                        	mov    rcx,rdi
  5b7435:	ff 15 25 70 32 03                               	call   QWORD PTR [rip+0x3327025]        # 0x38de460
  5b743b:	8b 44 24 60                                     	mov    eax,DWORD PTR [rsp+0x60]
  5b743f:	48 8d 0d 62 96 33 03                            	lea    rcx,[rip+0x3339662]        # 0x38f0aa8
  5b7446:	48 89 0b                                        	mov    QWORD PTR [rbx],rcx
  5b7449:	48 8b cf                                        	mov    rcx,rdi
  5b744c:	89 43 10                                        	mov    DWORD PTR [rbx+0x10],eax
  5b744f:	48 89 7b 18                                     	mov    QWORD PTR [rbx+0x18],rdi
  5b7453:	48 89 73 20                                     	mov    QWORD PTR [rbx+0x20],rsi
  5b7457:	c7 43 08 01 00 00 00                            	mov    DWORD PTR [rbx+0x8],0x1
  5b745e:	ff 15 0c 70 32 03                               	call   QWORD PTR [rip+0x332700c]        # 0x38de470
  5b7464:	48 8b bc 24 90 00 00 00                         	mov    rdi,QWORD PTR [rsp+0x90]
  5b746c:	48 85 ff                                        	test   rdi,rdi
  5b746f:	74 29                                           	je     0x5b749a
  5b7471:	48 83 c8 ff                                     	or     rax,0xffffffffffffffff
  5b7475:	48 ff c0                                        	inc    rax
  5b7478:	44 38 3c 07                                     	cmp    BYTE PTR [rdi+rax*1],r15b
  5b747c:	75 f7                                           	jne    0x5b7475
  5b747e:	48 85 c0                                        	test   rax,rax
  5b7481:	74 17                                           	je     0x5b749a
  5b7483:	8b 4b 10                                        	mov    ecx,DWORD PTR [rbx+0x10]
  5b7486:	48 8b d7                                        	mov    rdx,rdi
  5b7489:	e8 0a 58 4a 00                                  	call   0xa5cc98
  5b748e:	48 8b 4b 18                                     	mov    rcx,QWORD PTR [rbx+0x18]
  5b7492:	48 8b d7                                        	mov    rdx,rdi
  5b7495:	e8 0e ba 45 00                                  	call   0xa12ea8
  5b749a:	48 8b c3                                        	mov    rax,rbx
  5b749d:	48 8b 5c 24 68                                  	mov    rbx,QWORD PTR [rsp+0x68]
  5b74a2:	48 8b 6c 24 70                                  	mov    rbp,QWORD PTR [rsp+0x70]
  5b74a7:	48 8b 74 24 78                                  	mov    rsi,QWORD PTR [rsp+0x78]
  5b74ac:	48 83 c4 40                                     	add    rsp,0x40
  5b74b0:	41 5f                                           	pop    r15
  5b74b2:	41 5e                                           	pop    r14
  5b74b4:	5f                                              	pop    rdi
  5b74b5:	c3                                              	ret
