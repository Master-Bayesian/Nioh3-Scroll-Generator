
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b6c98 <.data>:
  5b6c98:	48 8b c4                                        	mov    rax,rsp
  5b6c9b:	48 89 58 10                                     	mov    QWORD PTR [rax+0x10],rbx
  5b6c9f:	48 89 70 18                                     	mov    QWORD PTR [rax+0x18],rsi
  5b6ca3:	48 89 78 20                                     	mov    QWORD PTR [rax+0x20],rdi
  5b6ca7:	55                                              	push   rbp
  5b6ca8:	41 54                                           	push   r12
  5b6caa:	41 55                                           	push   r13
  5b6cac:	41 56                                           	push   r14
  5b6cae:	41 57                                           	push   r15
  5b6cb0:	48 8d a8 58 fe ff ff                            	lea    rbp,[rax-0x1a8]
  5b6cb7:	48 81 ec 80 02 00 00                            	sub    rsp,0x280
  5b6cbe:	48 8b 05 6b d2 ef 03                            	mov    rax,QWORD PTR [rip+0x3efd26b]        # 0x44b3f30
  5b6cc5:	48 33 c4                                        	xor    rax,rsp
  5b6cc8:	48 89 85 70 01 00 00                            	mov    QWORD PTR [rbp+0x170],rax
  5b6ccf:	4c 8b b9 e8 00 00 00                            	mov    r15,QWORD PTR [rcx+0xe8]
  5b6cd6:	45 33 e4                                        	xor    r12d,r12d
  5b6cd9:	44 89 a1 b8 00 00 00                            	mov    DWORD PTR [rcx+0xb8],r12d
  5b6ce0:	48 8b f9                                        	mov    rdi,rcx
  5b6ce3:	88 54 24 30                                     	mov    BYTE PTR [rsp+0x30],dl
  5b6ce7:	e8 e4 0d a0 00                                  	call   0xfb7ad0
  5b6cec:	48 8b c8                                        	mov    rcx,rax
  5b6cef:	48 8b d8                                        	mov    rbx,rax
  5b6cf2:	e8 c5 10 00 00                                  	call   0x5b7dbc
  5b6cf7:	84 c0                                           	test   al,al
  5b6cf9:	75 11                                           	jne    0x5b6d0c
  5b6cfb:	c7 87 b8 00 00 00 0d 00 00 00                   	mov    DWORD PTR [rdi+0xb8],0xd
  5b6d05:	32 c0                                           	xor    al,al
  5b6d07:	e9 d2 01 00 00                                  	jmp    0x5b6ede
  5b6d0c:	4c 8d 4c 24 40                                  	lea    r9,[rsp+0x40]
  5b6d11:	4c 89 64 24 38                                  	mov    QWORD PTR [rsp+0x38],r12
  5b6d16:	4c 8d 44 24 48                                  	lea    r8,[rsp+0x48]
  5b6d1b:	4c 89 64 24 48                                  	mov    QWORD PTR [rsp+0x48],r12
  5b6d20:	48 8d 54 24 38                                  	lea    rdx,[rsp+0x38]
  5b6d25:	4c 89 64 24 40                                  	mov    QWORD PTR [rsp+0x40],r12
  5b6d2a:	48 8b cb                                        	mov    rcx,rbx
  5b6d2d:	ff 15 d5 76 32 03                               	call   QWORD PTR [rip+0x33276d5]        # 0x38de408
  5b6d33:	49 8d 87 58 01 10 00                            	lea    rax,[r15+0x100158]
  5b6d3a:	48 39 44 24 38                                  	cmp    QWORD PTR [rsp+0x38],rax
  5b6d3f:	77 0c                                           	ja     0x5b6d4d
  5b6d41:	c7 87 b8 00 00 00 06 00 00 00                   	mov    DWORD PTR [rdi+0xb8],0x6
  5b6d4b:	eb b8                                           	jmp    0x5b6d05
  5b6d4d:	e8 56 0b 00 00                                  	call   0x5b78a8
  5b6d52:	4c 8d 44 24 50                                  	lea    r8,[rsp+0x50]
  5b6d57:	c7 44 24 50 35 00 00 00                         	mov    DWORD PTR [rsp+0x50],0x35
  5b6d5f:	49 8d 97 58 01 00 00                            	lea    rdx,[r15+0x158]
  5b6d66:	4c 89 64 24 58                                  	mov    QWORD PTR [rsp+0x58],r12
  5b6d6b:	48 8b f0                                        	mov    rsi,rax
  5b6d6e:	48 8b 08                                        	mov    rcx,QWORD PTR [rax]
  5b6d71:	4c 8b 49 28                                     	mov    r9,QWORD PTR [rcx+0x28]
  5b6d75:	48 8b c8                                        	mov    rcx,rax
  5b6d78:	41 ff d1                                        	call   r9
  5b6d7b:	4c 8b f0                                        	mov    r14,rax
  5b6d7e:	48 85 c0                                        	test   rax,rax
  5b6d81:	0f 84 47 01 00 00                               	je     0x5b6ece
  5b6d87:	48 8b d0                                        	mov    rdx,rax
  5b6d8a:	48 8b cf                                        	mov    rcx,rdi
  5b6d8d:	e8 ea 10 00 00                                  	call   0x5b7e7c
  5b6d92:	85 c0                                           	test   eax,eax
  5b6d94:	0f 85 20 01 00 00                               	jne    0x5b6eba
  5b6d9a:	4c 8d a7 b0 00 00 00                            	lea    r12,[rdi+0xb0]
  5b6da1:	49 8b 1c 24                                     	mov    rbx,QWORD PTR [r12]
  5b6da5:	e8 26 0d a0 00                                  	call   0xfb7ad0
  5b6daa:	4c 8d af dc 0d 00 00                            	lea    r13,[rdi+0xddc]
  5b6db1:	4c 8b c8                                        	mov    r9,rax
  5b6db4:	4c 89 6c 24 28                                  	mov    QWORD PTR [rsp+0x28],r13
  5b6db9:	4c 8d 05 10 fe 4d 03                            	lea    r8,[rip+0x34dfe10]        # 0x3a96bd0
  5b6dc0:	ba 05 01 00 00                                  	mov    edx,0x105
  5b6dc5:	48 89 5c 24 20                                  	mov    QWORD PTR [rsp+0x20],rbx
  5b6dca:	48 8d 4c 24 60                                  	lea    rcx,[rsp+0x60]
  5b6dcf:	e8 e0 db d5 ff                                  	call   0x3149b4
  5b6dd4:	33 c0                                           	xor    eax,eax
  5b6dd6:	38 05 50 f2 ff 03                               	cmp    BYTE PTR [rip+0x3fff250],al        # 0x45b602c
  5b6ddc:	75 61                                           	jne    0x5b6e3f
  5b6dde:	38 87 27 0e 00 00                               	cmp    BYTE PTR [rdi+0xe27],al
  5b6de4:	74 59                                           	je     0x5b6e3f
  5b6de6:	38 44 24 30                                     	cmp    BYTE PTR [rsp+0x30],al
  5b6dea:	75 53                                           	jne    0x5b6e3f
  5b6dec:	4d 8b cc                                        	mov    r9,r12
  5b6def:	4c 89 6c 24 20                                  	mov    QWORD PTR [rsp+0x20],r13
  5b6df4:	e8 07 0b a0 00                                  	call   0xfb7900
  5b6df9:	48 8d 05 40 7c 5b 04                            	lea    rax,[rip+0x45b7c40]        # 0x4b6ea40
  5b6e00:	4c 8d 44 24 60                                  	lea    r8,[rsp+0x60]
  5b6e05:	4c 2b c0                                        	sub    r8,rax
  5b6e08:	0f b7 08                                        	movzx  ecx,WORD PTR [rax]
  5b6e0b:	42 0f b7 14 00                                  	movzx  edx,WORD PTR [rax+r8*1]
  5b6e10:	2b ca                                           	sub    ecx,edx
  5b6e12:	75 08                                           	jne    0x5b6e1c
  5b6e14:	48 83 c0 02                                     	add    rax,0x2
  5b6e18:	85 d2                                           	test   edx,edx
  5b6e1a:	75 ec                                           	jne    0x5b6e08
  5b6e1c:	45 33 e4                                        	xor    r12d,r12d
  5b6e1f:	85 c9                                           	test   ecx,ecx
  5b6e21:	75 1f                                           	jne    0x5b6e42
  5b6e23:	c6 87 28 0e 00 00 01                            	mov    BYTE PTR [rdi+0xe28],0x1
  5b6e2a:	c6 05 fb f1 ff 03 01                            	mov    BYTE PTR [rip+0x3fff1fb],0x1        # 0x45b602c
  5b6e31:	ff 15 61 75 32 03                               	call   QWORD PTR [rip+0x3327561]        # 0x38de398
  5b6e37:	89 05 f3 f1 ff 03                               	mov    DWORD PTR [rip+0x3fff1f3],eax        # 0x45b6030
  5b6e3d:	eb 03                                           	jmp    0x5b6e42
  5b6e3f:	45 33 e4                                        	xor    r12d,r12d
  5b6e42:	48 8b 97 c0 00 00 00                            	mov    rdx,QWORD PTR [rdi+0xc0]
  5b6e49:	48 8d 44 24 34                                  	lea    rax,[rsp+0x34]
  5b6e4e:	4d 8d 8f 58 01 00 00                            	lea    r9,[r15+0x158]
  5b6e55:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
  5b6e5a:	4d 8b c6                                        	mov    r8,r14
  5b6e5d:	44 89 64 24 34                                  	mov    DWORD PTR [rsp+0x34],r12d
  5b6e62:	48 8d 4c 24 60                                  	lea    rcx,[rsp+0x60]
  5b6e67:	e8 78 0c 00 00                                  	call   0x5b7ae4
  5b6e6c:	44 38 25 b9 f1 ff 03                            	cmp    BYTE PTR [rip+0x3fff1b9],r12b        # 0x45b602c
  5b6e73:	8a d8                                           	mov    bl,al
  5b6e75:	75 2c                                           	jne    0x5b6ea3
  5b6e77:	44 38 a7 27 0e 00 00                            	cmp    BYTE PTR [rdi+0xe27],r12b
  5b6e7e:	74 23                                           	je     0x5b6ea3
  5b6e80:	44 38 64 24 30                                  	cmp    BYTE PTR [rsp+0x30],r12b
  5b6e85:	75 1c                                           	jne    0x5b6ea3
  5b6e87:	84 c0                                           	test   al,al
  5b6e89:	75 18                                           	jne    0x5b6ea3
  5b6e8b:	8b 44 24 34                                     	mov    eax,DWORD PTR [rsp+0x34]
  5b6e8f:	89 05 9b f1 ff 03                               	mov    DWORD PTR [rip+0x3fff19b],eax        # 0x45b6030
  5b6e95:	c6 87 28 0e 00 00 01                            	mov    BYTE PTR [rdi+0xe28],0x1
  5b6e9c:	c6 05 89 f1 ff 03 01                            	mov    BYTE PTR [rip+0x3fff189],0x1        # 0x45b602c
  5b6ea3:	48 8b 06                                        	mov    rax,QWORD PTR [rsi]
  5b6ea6:	49 8b d6                                        	mov    rdx,r14
  5b6ea9:	48 8b ce                                        	mov    rcx,rsi
  5b6eac:	ff 50 58                                        	call   QWORD PTR [rax+0x58]
  5b6eaf:	f6 db                                           	neg    bl
  5b6eb1:	1b c0                                           	sbb    eax,eax
  5b6eb3:	f7 d0                                           	not    eax
  5b6eb5:	83 e0 0c                                        	and    eax,0xc
  5b6eb8:	eb 19                                           	jmp    0x5b6ed3
  5b6eba:	48 8b 06                                        	mov    rax,QWORD PTR [rsi]
  5b6ebd:	49 8b d6                                        	mov    rdx,r14
  5b6ec0:	48 8b ce                                        	mov    rcx,rsi
  5b6ec3:	ff 50 58                                        	call   QWORD PTR [rax+0x58]
  5b6ec6:	8b 87 b8 00 00 00                               	mov    eax,DWORD PTR [rdi+0xb8]
  5b6ecc:	eb 0b                                           	jmp    0x5b6ed9
  5b6ece:	b8 04 00 00 00                                  	mov    eax,0x4
  5b6ed3:	89 87 b8 00 00 00                               	mov    DWORD PTR [rdi+0xb8],eax
  5b6ed9:	85 c0                                           	test   eax,eax
  5b6edb:	0f 94 c0                                        	sete   al
  5b6ede:	48 8b 8d 70 01 00 00                            	mov    rcx,QWORD PTR [rbp+0x170]
  5b6ee5:	48 33 cc                                        	xor    rcx,rsp
  5b6ee8:	e8 c3 a2 5d 00                                  	call   0xb911b0
  5b6eed:	4c 8d 9c 24 80 02 00 00                         	lea    r11,[rsp+0x280]
  5b6ef5:	49 8b 5b 38                                     	mov    rbx,QWORD PTR [r11+0x38]
  5b6ef9:	49 8b 73 40                                     	mov    rsi,QWORD PTR [r11+0x40]
  5b6efd:	49 8b 7b 48                                     	mov    rdi,QWORD PTR [r11+0x48]
  5b6f01:	49 8b e3                                        	mov    rsp,r11
  5b6f04:	41 5f                                           	pop    r15
  5b6f06:	41 5e                                           	pop    r14
  5b6f08:	41 5d                                           	pop    r13
  5b6f0a:	41 5c                                           	pop    r12
  5b6f0c:	5d                                              	pop    rbp
  5b6f0d:	c3                                              	ret
