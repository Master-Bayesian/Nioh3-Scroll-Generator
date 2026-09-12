
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b7ae4 <.data>:
  5b7ae4:	40 55                                           	rex push rbp
  5b7ae6:	53                                              	push   rbx
  5b7ae7:	56                                              	push   rsi
  5b7ae8:	57                                              	push   rdi
  5b7ae9:	41 54                                           	push   r12
  5b7aeb:	41 55                                           	push   r13
  5b7aed:	41 56                                           	push   r14
  5b7aef:	41 57                                           	push   r15
  5b7af1:	48 8d ac 24 68 f8 ff ff                         	lea    rbp,[rsp-0x798]
  5b7af9:	48 81 ec 98 08 00 00                            	sub    rsp,0x898
  5b7b00:	48 8b 05 29 c4 ef 03                            	mov    rax,QWORD PTR [rip+0x3efc429]        # 0x44b3f30
  5b7b07:	48 33 c4                                        	xor    rax,rsp
  5b7b0a:	48 89 85 80 07 00 00                            	mov    QWORD PTR [rbp+0x780],rax
  5b7b11:	48 8b 9d 00 08 00 00                            	mov    rbx,QWORD PTR [rbp+0x800]
  5b7b18:	49 8b f9                                        	mov    rdi,r9
  5b7b1b:	4d 8b f8                                        	mov    r15,r8
  5b7b1e:	4c 8b ea                                        	mov    r13,rdx
  5b7b21:	4c 8b f1                                        	mov    r14,rcx
  5b7b24:	83 23 00                                        	and    DWORD PTR [rbx],0x0
  5b7b27:	48 85 c9                                        	test   rcx,rcx
  5b7b2a:	74 4f                                           	je     0x5b7b7b
  5b7b2c:	48 85 d2                                        	test   rdx,rdx
  5b7b2f:	74 4a                                           	je     0x5b7b7b
  5b7b31:	4d 85 c0                                        	test   r8,r8
  5b7b34:	74 45                                           	je     0x5b7b7b
  5b7b36:	4d 85 c9                                        	test   r9,r9
  5b7b39:	74 40                                           	je     0x5b7b7b
  5b7b3b:	48 89 54 24 20                                  	mov    QWORD PTR [rsp+0x20],rdx
  5b7b40:	4c 8d 0d 89 e9 4d 03                            	lea    r9,[rip+0x34de989]        # 0x3a964d0
  5b7b47:	ba 04 01 00 00                                  	mov    edx,0x104
  5b7b4c:	48 8d 4c 24 40                                  	lea    rcx,[rsp+0x40]
  5b7b51:	49 83 c8 ff                                     	or     r8,0xffffffffffffffff
  5b7b55:	e8 da 01 00 00                                  	call   0x5b7d34
  5b7b5a:	48 8d 54 24 40                                  	lea    rdx,[rsp+0x40]
  5b7b5f:	49 8b ce                                        	mov    rcx,r14
  5b7b62:	45 32 e4                                        	xor    r12b,r12b
  5b7b65:	e8 aa fe ff ff                                  	call   0x5b7a14
  5b7b6a:	48 8b f0                                        	mov    rsi,rax
  5b7b6d:	48 83 f8 ff                                     	cmp    rax,0xffffffffffffffff
  5b7b71:	75 2d                                           	jne    0x5b7ba0
  5b7b73:	ff 15 1f 68 32 03                               	call   QWORD PTR [rip+0x332681f]        # 0x38de398
  5b7b79:	89 03                                           	mov    DWORD PTR [rbx],eax
  5b7b7b:	32 c0                                           	xor    al,al
  5b7b7d:	48 8b 8d 80 07 00 00                            	mov    rcx,QWORD PTR [rbp+0x780]
  5b7b84:	48 33 cc                                        	xor    rcx,rsp
  5b7b87:	e8 24 96 5d 00                                  	call   0xb911b0
  5b7b8c:	48 81 c4 98 08 00 00                            	add    rsp,0x898
  5b7b93:	41 5f                                           	pop    r15
  5b7b95:	41 5e                                           	pop    r14
  5b7b97:	41 5d                                           	pop    r13
  5b7b99:	41 5c                                           	pop    r12
  5b7b9b:	5f                                              	pop    rdi
  5b7b9c:	5e                                              	pop    rsi
  5b7b9d:	5b                                              	pop    rbx
  5b7b9e:	5d                                              	pop    rbp
  5b7b9f:	c3                                              	ret
  5b7ba0:	83 64 24 30 00                                  	and    DWORD PTR [rsp+0x30],0x0
  5b7ba5:	4c 8d 4c 24 30                                  	lea    r9,[rsp+0x30]
  5b7baa:	48 83 64 24 20 00                               	and    QWORD PTR [rsp+0x20],0x0
  5b7bb0:	44 8b c7                                        	mov    r8d,edi
  5b7bb3:	49 8b d7                                        	mov    rdx,r15
  5b7bb6:	48 8b ce                                        	mov    rcx,rsi
  5b7bb9:	ff 15 c1 67 32 03                               	call   QWORD PTR [rip+0x33267c1]        # 0x38de380
  5b7bbf:	85 c0                                           	test   eax,eax
  5b7bc1:	74 2b                                           	je     0x5b7bee
  5b7bc3:	8b 44 24 30                                     	mov    eax,DWORD PTR [rsp+0x30]
  5b7bc7:	48 3b f8                                        	cmp    rdi,rax
  5b7bca:	74 08                                           	je     0x5b7bd4
  5b7bcc:	ff 15 c6 67 32 03                               	call   QWORD PTR [rip+0x33267c6]        # 0x38de398
  5b7bd2:	89 03                                           	mov    DWORD PTR [rbx],eax
  5b7bd4:	48 8b ce                                        	mov    rcx,rsi
  5b7bd7:	ff 15 ab 67 32 03                               	call   QWORD PTR [rip+0x33267ab]        # 0x38de388
  5b7bdd:	85 c0                                           	test   eax,eax
  5b7bdf:	74 0d                                           	je     0x5b7bee
  5b7be1:	8b 44 24 30                                     	mov    eax,DWORD PTR [rsp+0x30]
  5b7be5:	48 3b f8                                        	cmp    rdi,rax
  5b7be8:	41 0f 94 c4                                     	sete   r12b
  5b7bec:	eb 08                                           	jmp    0x5b7bf6
  5b7bee:	ff 15 a4 67 32 03                               	call   QWORD PTR [rip+0x33267a4]        # 0x38de398
  5b7bf4:	89 03                                           	mov    DWORD PTR [rbx],eax
  5b7bf6:	48 8b ce                                        	mov    rcx,rsi
  5b7bf9:	ff 15 91 67 32 03                               	call   QWORD PTR [rip+0x3326791]        # 0x38de390
  5b7bff:	4c 8d 0d b2 29 33 03                            	lea    r9,[rip+0x33329b2]        # 0x38ea5b8
  5b7c06:	45 84 e4                                        	test   r12b,r12b
  5b7c09:	75 35                                           	jne    0x5b7c40
  5b7c0b:	48 8d 44 24 40                                  	lea    rax,[rsp+0x40]
  5b7c10:	49 83 c8 ff                                     	or     r8,0xffffffffffffffff
  5b7c14:	48 89 44 24 28                                  	mov    QWORD PTR [rsp+0x28],rax
  5b7c19:	48 8d 8d 70 05 00 00                            	lea    rcx,[rbp+0x570]
  5b7c20:	ba 04 01 00 00                                  	mov    edx,0x104
  5b7c25:	4c 89 74 24 20                                  	mov    QWORD PTR [rsp+0x20],r14
  5b7c2a:	e8 05 01 00 00                                  	call   0x5b7d34
  5b7c2f:	48 8d 8d 70 05 00 00                            	lea    rcx,[rbp+0x570]
  5b7c36:	e8 31 e4 a1 01                                  	call   0x1fd606c
  5b7c3b:	e9 3b ff ff ff                                  	jmp    0x5b7b7b
  5b7c40:	48 83 cf ff                                     	or     rdi,0xffffffffffffffff
  5b7c44:	4c 89 6c 24 28                                  	mov    QWORD PTR [rsp+0x28],r13
  5b7c49:	be 04 01 00 00                                  	mov    esi,0x104
  5b7c4e:	4c 89 74 24 20                                  	mov    QWORD PTR [rsp+0x20],r14
  5b7c53:	4c 8b c7                                        	mov    r8,rdi
  5b7c56:	48 8d 8d 50 01 00 00                            	lea    rcx,[rbp+0x150]
  5b7c5d:	8b d6                                           	mov    edx,esi
  5b7c5f:	e8 d0 00 00 00                                  	call   0x5b7d34
  5b7c64:	48 8d 44 24 40                                  	lea    rax,[rsp+0x40]
  5b7c69:	4c 8b c7                                        	mov    r8,rdi
  5b7c6c:	48 89 44 24 28                                  	mov    QWORD PTR [rsp+0x28],rax
  5b7c71:	4c 8d 0d 40 29 33 03                            	lea    r9,[rip+0x3332940]        # 0x38ea5b8
  5b7c78:	8b d6                                           	mov    edx,esi
  5b7c7a:	4c 89 74 24 20                                  	mov    QWORD PTR [rsp+0x20],r14
  5b7c7f:	48 8d 8d 60 03 00 00                            	lea    rcx,[rbp+0x360]
  5b7c86:	e8 a9 00 00 00                                  	call   0x5b7d34
  5b7c8b:	48 8d 8d 50 01 00 00                            	lea    rcx,[rbp+0x150]
  5b7c92:	ff 15 d0 66 32 03                               	call   QWORD PTR [rip+0x33266d0]        # 0x38de368
  5b7c98:	83 f8 ff                                        	cmp    eax,0xffffffff
  5b7c9b:	74 16                                           	je     0x5b7cb3
  5b7c9d:	a8 01                                           	test   al,0x1
  5b7c9f:	74 12                                           	je     0x5b7cb3
  5b7ca1:	83 f0 01                                        	xor    eax,0x1
  5b7ca4:	48 8d 8d 50 01 00 00                            	lea    rcx,[rbp+0x150]
  5b7cab:	8b d0                                           	mov    edx,eax
  5b7cad:	ff 15 bd 66 32 03                               	call   QWORD PTR [rip+0x33266bd]        # 0x38de370
  5b7cb3:	41 b8 09 00 00 00                               	mov    r8d,0x9
  5b7cb9:	48 8d 95 50 01 00 00                            	lea    rdx,[rbp+0x150]
  5b7cc0:	48 8d 8d 60 03 00 00                            	lea    rcx,[rbp+0x360]
  5b7cc7:	ff 15 d3 66 32 03                               	call   QWORD PTR [rip+0x33266d3]        # 0x38de3a0
  5b7ccd:	85 c0                                           	test   eax,eax
  5b7ccf:	75 14                                           	jne    0x5b7ce5
  5b7cd1:	ff 15 c1 66 32 03                               	call   QWORD PTR [rip+0x33266c1]        # 0x38de398
  5b7cd7:	89 03                                           	mov    DWORD PTR [rbx],eax
  5b7cd9:	48 8d 8d 60 03 00 00                            	lea    rcx,[rbp+0x360]
  5b7ce0:	e9 51 ff ff ff                                  	jmp    0x5b7c36
  5b7ce5:	b0 01                                           	mov    al,0x1
  5b7ce7:	e9 91 fe ff ff                                  	jmp    0x5b7b7d
