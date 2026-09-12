
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b8acc <.data>:
  5b8acc:	48 89 5c 24 08                                  	mov    QWORD PTR [rsp+0x8],rbx
  5b8ad1:	55                                              	push   rbp
  5b8ad2:	48 8d 6c 24 a9                                  	lea    rbp,[rsp-0x57]
  5b8ad7:	48 81 ec b0 00 00 00                            	sub    rsp,0xb0
  5b8ade:	33 c0                                           	xor    eax,eax
  5b8ae0:	48 89 4d c7                                     	mov    QWORD PTR [rbp-0x39],rcx
  5b8ae4:	48 89 55 cf                                     	mov    QWORD PTR [rbp-0x31],rdx
  5b8ae8:	0f 10 45 c7                                     	movups xmm0,XMMWORD PTR [rbp-0x39]
  5b8aec:	48 89 45 d7                                     	mov    QWORD PTR [rbp-0x29],rax
  5b8af0:	f2 0f 10 4d d7                                  	movsd  xmm1,QWORD PTR [rbp-0x29]
  5b8af5:	48 89 4d 17                                     	mov    QWORD PTR [rbp+0x17],rcx
  5b8af9:	48 8d 4d e7                                     	lea    rcx,[rbp-0x19]
  5b8afd:	0f 29 45 e7                                     	movaps XMMWORD PTR [rbp-0x19],xmm0
  5b8b01:	f2 0f 11 4d f7                                  	movsd  QWORD PTR [rbp-0x9],xmm1
  5b8b06:	c6 45 0f 01                                     	mov    BYTE PTR [rbp+0xf],0x1
  5b8b0a:	88 45 37                                        	mov    BYTE PTR [rbp+0x37],al
  5b8b0d:	89 45 3f                                        	mov    DWORD PTR [rbp+0x3f],eax
  5b8b10:	88 45 4b                                        	mov    BYTE PTR [rbp+0x4b],al
  5b8b13:	e8 cc 13 00 00                                  	call   0x5b9ee4
  5b8b18:	48 8d 4d e7                                     	lea    rcx,[rbp-0x19]
  5b8b1c:	e8 bb 13 00 00                                  	call   0x5b9edc
  5b8b21:	48 8b c8                                        	mov    rcx,rax
  5b8b24:	e8 07 2c 42 00                                  	call   0x9db730
  5b8b29:	48 8d 55 e7                                     	lea    rdx,[rbp-0x19]
  5b8b2d:	48 8d 0d bc 83 ac 00                            	lea    rcx,[rip+0xac83bc]        # 0x1080ef0
  5b8b34:	e8 cf 00 00 00                                  	call   0x5b8c08
  5b8b39:	48 8d 4d e7                                     	lea    rcx,[rbp-0x19]
  5b8b3d:	e8 9e 00 00 00                                  	call   0x5b8be0
  5b8b42:	48 8d 4d e7                                     	lea    rcx,[rbp-0x19]
  5b8b46:	48 8b d8                                        	mov    rbx,rax
  5b8b49:	e8 16 00 00 00                                  	call   0x5b8b64
  5b8b4e:	48 8b c3                                        	mov    rax,rbx
  5b8b51:	48 8b 9c 24 c0 00 00 00                         	mov    rbx,QWORD PTR [rsp+0xc0]
  5b8b59:	48 81 c4 b0 00 00 00                            	add    rsp,0xb0
  5b8b60:	5d                                              	pop    rbp
  5b8b61:	c3                                              	ret
