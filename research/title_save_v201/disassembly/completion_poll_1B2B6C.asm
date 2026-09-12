
supplied-text-section: file format binary


Disassembly of section .data:

00000000001b2b6c <.data>:
  1b2b6c:	48 89 5c 24 10                                  	mov    QWORD PTR [rsp+0x10],rbx
  1b2b71:	48 89 74 24 18                                  	mov    QWORD PTR [rsp+0x18],rsi
  1b2b76:	55                                              	push   rbp
  1b2b77:	57                                              	push   rdi
  1b2b78:	41 56                                           	push   r14
  1b2b7a:	48 8d ac 24 b0 f5 ff ff                         	lea    rbp,[rsp-0xa50]
  1b2b82:	48 81 ec 50 0b 00 00                            	sub    rsp,0xb50
  1b2b89:	48 8b 05 a0 13 30 04                            	mov    rax,QWORD PTR [rip+0x43013a0]        # 0x44b3f30
  1b2b90:	48 33 c4                                        	xor    rax,rsp
  1b2b93:	48 89 85 40 0a 00 00                            	mov    QWORD PTR [rbp+0xa40],rax
  1b2b9a:	48 8b d9                                        	mov    rbx,rcx
  1b2b9d:	e8 7a 03 00 00                                  	call   0x1b2f1c
  1b2ba2:	45 33 f6                                        	xor    r14d,r14d
  1b2ba5:	84 c0                                           	test   al,al
  1b2ba7:	0f 84 47 03 00 00                               	je     0x1b2ef4
  1b2bad:	44 8a 43 39                                     	mov    r8b,BYTE PTR [rbx+0x39]
  1b2bb1:	45 84 c0                                        	test   r8b,r8b
  1b2bb4:	75 08                                           	jne    0x1b2bbe
  1b2bb6:	e8 a1 fe ff ff                                  	call   0x1b2a5c
  1b2bbb:	89 43 30                                        	mov    DWORD PTR [rbx+0x30],eax
  1b2bbe:	e8 b1 fe ff ff                                  	call   0x1b2a74
  1b2bc3:	89 43 18                                        	mov    DWORD PTR [rbx+0x18],eax
  1b2bc6:	8b d0                                           	mov    edx,eax
  1b2bc8:	83 f8 07                                        	cmp    eax,0x7
  1b2bcb:	0f 8f 5f 01 00 00                               	jg     0x1b2d30
  1b2bd1:	0f 84 83 01 00 00                               	je     0x1b2d5a
  1b2bd7:	85 c0                                           	test   eax,eax
  1b2bd9:	74 70                                           	je     0x1b2c4b
  1b2bdb:	83 ea 01                                        	sub    edx,0x1
  1b2bde:	74 5e                                           	je     0x1b2c3e
  1b2be0:	83 ea 01                                        	sub    edx,0x1
  1b2be3:	0f 84 32 01 00 00                               	je     0x1b2d1b
  1b2be9:	83 ea 01                                        	sub    edx,0x1
  1b2bec:	0f 84 68 01 00 00                               	je     0x1b2d5a
  1b2bf2:	83 ea 01                                        	sub    edx,0x1
  1b2bf5:	0f 84 65 01 00 00                               	je     0x1b2d60
  1b2bfb:	83 ea 01                                        	sub    edx,0x1
  1b2bfe:	0f 84 5c 01 00 00                               	je     0x1b2d60
  1b2c04:	83 fa 01                                        	cmp    edx,0x1
  1b2c07:	0f 85 53 01 00 00                               	jne    0x1b2d60
  1b2c0d:	88 53 3d                                        	mov    BYTE PTR [rbx+0x3d],dl
  1b2c10:	4c 89 73 20                                     	mov    QWORD PTR [rbx+0x20],r14
  1b2c14:	44 88 73 3b                                     	mov    BYTE PTR [rbx+0x3b],r14b
  1b2c18:	e8 87 fe ff ff                                  	call   0x1b2aa4
  1b2c1d:	a9 fd ff ff ff                                  	test   eax,0xfffffffd
  1b2c22:	0f 85 38 01 00 00                               	jne    0x1b2d60
  1b2c28:	45 84 c0                                        	test   r8b,r8b
  1b2c2b:	0f 85 2f 01 00 00                               	jne    0x1b2d60
  1b2c31:	e8 56 fe ff ff                                  	call   0x1b2a8c
  1b2c36:	89 43 28                                        	mov    DWORD PTR [rbx+0x28],eax
  1b2c39:	e9 22 01 00 00                                  	jmp    0x1b2d60
  1b2c3e:	c6 43 3a 01                                     	mov    BYTE PTR [rbx+0x3a],0x1
  1b2c42:	c6 43 3f 01                                     	mov    BYTE PTR [rbx+0x3f],0x1
  1b2c46:	e9 15 01 00 00                                  	jmp    0x1b2d60
  1b2c4b:	e8 3c fe ff ff                                  	call   0x1b2a8c
  1b2c50:	8b f0                                           	mov    esi,eax
  1b2c52:	e8 4d fe ff ff                                  	call   0x1b2aa4
  1b2c57:	8b f8                                           	mov    edi,eax
  1b2c59:	8d 48 ff                                        	lea    ecx,[rax-0x1]
  1b2c5c:	f7 c1 fd ff ff ff                               	test   ecx,0xfffffffd
  1b2c62:	74 05                                           	je     0x1b2c69
  1b2c64:	41 8a ce                                        	mov    cl,r14b
  1b2c67:	eb 0c                                           	jmp    0x1b2c75
  1b2c69:	83 ff 01                                        	cmp    edi,0x1
  1b2c6c:	74 0c                                           	je     0x1b2c7a
  1b2c6e:	b1 01                                           	mov    cl,0x1
  1b2c70:	83 ff 03                                        	cmp    edi,0x3
  1b2c73:	74 05                                           	je     0x1b2c7a
  1b2c75:	41 8a c6                                        	mov    al,r14b
  1b2c78:	eb 04                                           	jmp    0x1b2c7e
  1b2c7a:	b0 01                                           	mov    al,0x1
  1b2c7c:	8a c8                                           	mov    cl,al
  1b2c7e:	c6 43 3a 01                                     	mov    BYTE PTR [rbx+0x3a],0x1
  1b2c82:	84 c9                                           	test   cl,cl
  1b2c84:	75 04                                           	jne    0x1b2c8a
  1b2c86:	85 ff                                           	test   edi,edi
  1b2c88:	75 0c                                           	jne    0x1b2c96
  1b2c8a:	45 84 c0                                        	test   r8b,r8b
  1b2c8d:	75 07                                           	jne    0x1b2c96
  1b2c8f:	c6 43 3b 01                                     	mov    BYTE PTR [rbx+0x3b],0x1
  1b2c93:	89 73 28                                        	mov    DWORD PTR [rbx+0x28],esi
  1b2c96:	44 39 73 34                                     	cmp    DWORD PTR [rbx+0x34],r14d
  1b2c9a:	74 1f                                           	je     0x1b2cbb
  1b2c9c:	48 8b 43 08                                     	mov    rax,QWORD PTR [rbx+0x8]
  1b2ca0:	48 85 c0                                        	test   rax,rax
  1b2ca3:	0f 84 b7 00 00 00                               	je     0x1b2d60
  1b2ca9:	44 39 b0 10 98 03 00                            	cmp    DWORD PTR [rax+0x39810],r14d
  1b2cb0:	0f 95 c0                                        	setne  al
  1b2cb3:	88 43 40                                        	mov    BYTE PTR [rbx+0x40],al
  1b2cb6:	e9 a5 00 00 00                                  	jmp    0x1b2d60
  1b2cbb:	45 84 c0                                        	test   r8b,r8b
  1b2cbe:	75 08                                           	jne    0x1b2cc8
  1b2cc0:	83 ff 05                                        	cmp    edi,0x5
  1b2cc3:	74 03                                           	je     0x1b2cc8
  1b2cc5:	89 73 28                                        	mov    DWORD PTR [rbx+0x28],esi
  1b2cc8:	84 c0                                           	test   al,al
  1b2cca:	75 05                                           	jne    0x1b2cd1
  1b2ccc:	83 ff 05                                        	cmp    edi,0x5
  1b2ccf:	75 40                                           	jne    0x1b2d11
  1b2cd1:	45 84 c0                                        	test   r8b,r8b
  1b2cd4:	74 0f                                           	je     0x1b2ce5
  1b2cd6:	4c 39 73 08                                     	cmp    QWORD PTR [rbx+0x8],r14
  1b2cda:	74 25                                           	je     0x1b2d01
  1b2cdc:	81 7b 10 00 03 11 25                            	cmp    DWORD PTR [rbx+0x10],0x25110300
  1b2ce3:	eb 0c                                           	jmp    0x1b2cf1
  1b2ce5:	4c 39 33                                        	cmp    QWORD PTR [rbx],r14
  1b2ce8:	74 17                                           	je     0x1b2d01
  1b2cea:	81 7b 14 00 14 11 25                            	cmp    DWORD PTR [rbx+0x14],0x25111400
  1b2cf1:	75 0e                                           	jne    0x1b2d01
  1b2cf3:	84 c0                                           	test   al,al
  1b2cf5:	74 1a                                           	je     0x1b2d11
  1b2cf7:	48 8b cb                                        	mov    rcx,rbx
  1b2cfa:	e8 75 72 7c 02                                  	call   0x2979f74
  1b2cff:	eb 10                                           	jmp    0x1b2d11
  1b2d01:	66 44 89 73 3a                                  	mov    WORD PTR [rbx+0x3a],r14w
  1b2d06:	c6 43 3c 01                                     	mov    BYTE PTR [rbx+0x3c],0x1
  1b2d0a:	c7 43 18 07 00 00 00                            	mov    DWORD PTR [rbx+0x18],0x7
  1b2d11:	83 ff 04                                        	cmp    edi,0x4
  1b2d14:	75 0b                                           	jne    0x1b2d21
  1b2d16:	3b 73 28                                        	cmp    esi,DWORD PTR [rbx+0x28]
  1b2d19:	75 45                                           	jne    0x1b2d60
  1b2d1b:	44 88 73 3b                                     	mov    BYTE PTR [rbx+0x3b],r14b
  1b2d1f:	eb 3f                                           	jmp    0x1b2d60
  1b2d21:	83 ff 05                                        	cmp    edi,0x5
  1b2d24:	75 3a                                           	jne    0x1b2d60
  1b2d26:	e8 c9 5a e2 01                                  	call   0x1fd87f4
  1b2d2b:	88 43 3e                                        	mov    BYTE PTR [rbx+0x3e],al
  1b2d2e:	eb 30                                           	jmp    0x1b2d60
  1b2d30:	83 ea 08                                        	sub    edx,0x8
  1b2d33:	0f 84 a5 01 00 00                               	je     0x1b2ede
  1b2d39:	83 ea 01                                        	sub    edx,0x1
  1b2d3c:	74 30                                           	je     0x1b2d6e
  1b2d3e:	83 ea 01                                        	sub    edx,0x1
  1b2d41:	74 0a                                           	je     0x1b2d4d
  1b2d43:	83 ea 01                                        	sub    edx,0x1
  1b2d46:	74 18                                           	je     0x1b2d60
  1b2d48:	83 ea 02                                        	sub    edx,0x2
  1b2d4b:	eb 13                                           	jmp    0x1b2d60
  1b2d4d:	e8 2a ee f3 ff                                  	call   0xf1b7c
  1b2d52:	84 c0                                           	test   al,al
  1b2d54:	0f 85 9a 01 00 00                               	jne    0x1b2ef4
  1b2d5a:	66 c7 43 3b 00 01                               	mov    WORD PTR [rbx+0x3b],0x100
  1b2d60:	44 88 73 38                                     	mov    BYTE PTR [rbx+0x38],r14b
  1b2d64:	e8 53 fd ff ff                                  	call   0x1b2abc
  1b2d69:	e9 86 01 00 00                                  	jmp    0x1b2ef4
  1b2d6e:	48 8d 4c 24 50                                  	lea    rcx,[rsp+0x50]
  1b2d73:	e8 68 12 55 00                                  	call   0x703fe0
  1b2d78:	33 d2                                           	xor    edx,edx
  1b2d7a:	c7 44 24 50 01 00 00 00                         	mov    DWORD PTR [rsp+0x50],0x1
  1b2d82:	41 b8 00 08 00 00                               	mov    r8d,0x800
  1b2d88:	48 8d 8d 40 02 00 00                            	lea    rcx,[rbp+0x240]
  1b2d8f:	e8 3c 10 9e 00                                  	call   0xb93dd0
  1b2d94:	b9 00 04 00 00                                  	mov    ecx,0x400
  1b2d99:	4c 89 75 20                                     	mov    QWORD PTR [rbp+0x20],r14
  1b2d9d:	48 8d 85 40 02 00 00                            	lea    rax,[rbp+0x240]
  1b2da4:	48 89 4d 28                                     	mov    QWORD PTR [rbp+0x28],rcx
  1b2da8:	48 89 45 08                                     	mov    QWORD PTR [rbp+0x8],rax
  1b2dac:	48 8d 1d 85 69 74 03                            	lea    rbx,[rip+0x3746985]        # 0x38f9738
  1b2db3:	48 8d 85 40 02 00 00                            	lea    rax,[rbp+0x240]
  1b2dba:	48 89 4d 18                                     	mov    QWORD PTR [rbp+0x18],rcx
  1b2dbe:	b9 f2 bc 9a 01                                  	mov    ecx,0x19abcf2
  1b2dc3:	48 89 45 30                                     	mov    QWORD PTR [rbp+0x30],rax
  1b2dc7:	4c 89 75 10                                     	mov    QWORD PTR [rbp+0x10],r14
  1b2dcb:	48 89 5d 00                                     	mov    QWORD PTR [rbp+0x0],rbx
  1b2dcf:	e8 60 1c 3f 00                                  	call   0x5a4a34
  1b2dd4:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
  1b2dd9:	49 8b ce                                        	mov    rcx,r14
  1b2ddc:	0f 28 44 24 20                                  	movaps xmm0,XMMWORD PTR [rsp+0x20]
  1b2de1:	48 8d 44 24 40                                  	lea    rax,[rsp+0x40]
  1b2de6:	48 89 44 24 38                                  	mov    QWORD PTR [rsp+0x38],rax
  1b2deb:	48 8d 05 52 75 73 03                            	lea    rax,[rip+0x3737552]        # 0x38ea344
  1b2df2:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
  1b2df7:	66 0f 7f 44 24 40                               	movdqa XMMWORD PTR [rsp+0x40],xmm0
  1b2dfd:	48 c7 44 24 30 0c 00 00 00                      	mov    QWORD PTR [rsp+0x30],0xc
  1b2e06:	48 ff c1                                        	inc    rcx
  1b2e09:	48 8d 40 02                                     	lea    rax,[rax+0x2]
  1b2e0d:	66 44 39 30                                     	cmp    WORD PTR [rax],r14w
  1b2e11:	75 f3                                           	jne    0x1b2e06
  1b2e13:	0f 28 44 24 30                                  	movaps xmm0,XMMWORD PTR [rsp+0x30]
  1b2e18:	4c 8d 44 24 30                                  	lea    r8,[rsp+0x30]
  1b2e1d:	48 89 4c 24 28                                  	mov    QWORD PTR [rsp+0x28],rcx
  1b2e22:	48 8d 54 24 20                                  	lea    rdx,[rsp+0x20]
  1b2e27:	0f 28 4c 24 20                                  	movaps xmm1,XMMWORD PTR [rsp+0x20]
  1b2e2c:	48 8d 4d 00                                     	lea    rcx,[rbp+0x0]
  1b2e30:	66 0f 7f 4c 24 20                               	movdqa XMMWORD PTR [rsp+0x20],xmm1
  1b2e36:	66 0f 7f 44 24 30                               	movdqa XMMWORD PTR [rsp+0x30],xmm0
  1b2e3c:	e8 47 b4 15 00                                  	call   0x30e288
  1b2e41:	b9 ff 03 00 00                                  	mov    ecx,0x3ff
  1b2e46:	48 89 5d 00                                     	mov    QWORD PTR [rbp+0x0],rbx
  1b2e4a:	48 3b c1                                        	cmp    rax,rcx
  1b2e4d:	48 0f 47 c1                                     	cmova  rax,rcx
  1b2e51:	48 8d 4d 00                                     	lea    rcx,[rbp+0x0]
  1b2e55:	66 44 89 b4 45 40 02 00 00                      	mov    WORD PTR [rbp+rax*2+0x240],r14w
  1b2e5e:	e8 b5 53 80 00                                  	call   0x9b8218
  1b2e63:	48 8d 95 40 02 00 00                            	lea    rdx,[rbp+0x240]
  1b2e6a:	48 8d 4c 24 58                                  	lea    rcx,[rsp+0x58]
  1b2e6f:	e8 f8 d9 15 00                                  	call   0x31086c
  1b2e74:	b9 31 85 c6 03                                  	mov    ecx,0x3c68531
  1b2e79:	e8 b6 1b 3f 00                                  	call   0x5a4a34
  1b2e7e:	48 8b d0                                        	mov    rdx,rax
  1b2e81:	48 8d 4c 24 78                                  	lea    rcx,[rsp+0x78]
  1b2e86:	e8 e1 d9 15 00                                  	call   0x31086c
  1b2e8b:	48 8d 54 24 50                                  	lea    rdx,[rsp+0x50]
  1b2e90:	c7 45 e0 cf 06 d3 5c                            	mov    DWORD PTR [rbp-0x20],0x5cd306cf
  1b2e97:	48 8d 4d 00                                     	lea    rcx,[rbp+0x0]
  1b2e9b:	c7 45 dc bd c7 6e 22                            	mov    DWORD PTR [rbp-0x24],0x226ec7bd
  1b2ea2:	c7 45 d8 2d a4 d7 86                            	mov    DWORD PTR [rbp-0x28],0x86d7a42d
  1b2ea9:	e8 02 60 55 00                                  	call   0x708eb0
  1b2eae:	48 8b c8                                        	mov    rcx,rax
  1b2eb1:	0f 57 d2                                        	xorps  xmm2,xmm2
  1b2eb4:	83 ca ff                                        	or     edx,0xffffffff
  1b2eb7:	e8 c4 10 55 00                                  	call   0x703f80
  1b2ebc:	48 8b 05 bd ff 40 04                            	mov    rax,QWORD PTR [rip+0x440ffbd]        # 0x45c2e80
  1b2ec3:	48 85 c0                                        	test   rax,rax
  1b2ec6:	74 0a                                           	je     0x1b2ed2
  1b2ec8:	c7 80 b8 00 00 00 0a 00 00 00                   	mov    DWORD PTR [rax+0xb8],0xa
  1b2ed2:	48 8d 4c 24 50                                  	lea    rcx,[rsp+0x50]
  1b2ed7:	e8 08 14 55 00                                  	call   0x7042e4
  1b2edc:	eb 16                                           	jmp    0x1b2ef4
  1b2ede:	48 8b 05 9b ff 40 04                            	mov    rax,QWORD PTR [rip+0x440ff9b]        # 0x45c2e80
  1b2ee5:	48 85 c0                                        	test   rax,rax
  1b2ee8:	74 0a                                           	je     0x1b2ef4
  1b2eea:	c7 80 b8 00 00 00 09 00 00 00                   	mov    DWORD PTR [rax+0xb8],0x9
  1b2ef4:	48 8b 8d 40 0a 00 00                            	mov    rcx,QWORD PTR [rbp+0xa40]
  1b2efb:	48 33 cc                                        	xor    rcx,rsp
  1b2efe:	e8 ad e2 9d 00                                  	call   0xb911b0
  1b2f03:	4c 8d 9c 24 50 0b 00 00                         	lea    r11,[rsp+0xb50]
  1b2f0b:	49 8b 5b 28                                     	mov    rbx,QWORD PTR [r11+0x28]
  1b2f0f:	49 8b 73 30                                     	mov    rsi,QWORD PTR [r11+0x30]
  1b2f13:	49 8b e3                                        	mov    rsp,r11
  1b2f16:	41 5e                                           	pop    r14
  1b2f18:	5f                                              	pop    rdi
  1b2f19:	5d                                              	pop    rbp
  1b2f1a:	c3                                              	ret
