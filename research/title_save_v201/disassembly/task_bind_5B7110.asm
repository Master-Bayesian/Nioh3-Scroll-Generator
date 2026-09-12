
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b7110 <.data>:
  5b7110:	48 89 5c 24 08                                  	mov    QWORD PTR [rsp+0x8],rbx
  5b7115:	57                                              	push   rdi
  5b7116:	48 83 ec 20                                     	sub    rsp,0x20
  5b711a:	41 8b 40 04                                     	mov    eax,DWORD PTR [r8+0x4]
  5b711e:	48 8b d9                                        	mov    rbx,rcx
  5b7121:	41 0f 10 40 28                                  	movups xmm0,XMMWORD PTR [r8+0x28]
  5b7126:	48 89 81 d8 00 00 00                            	mov    QWORD PTR [rcx+0xd8],rax
  5b712d:	49 8b f8                                        	mov    rdi,r8
  5b7130:	49 8b 40 08                                     	mov    rax,QWORD PTR [r8+0x8]
  5b7134:	41 0f 10 48 38                                  	movups xmm1,XMMWORD PTR [r8+0x38]
  5b7139:	48 89 81 e0 00 00 00                            	mov    QWORD PTR [rcx+0xe0],rax
  5b7140:	49 8b 40 10                                     	mov    rax,QWORD PTR [r8+0x10]
  5b7144:	48 89 81 e8 00 00 00                            	mov    QWORD PTR [rcx+0xe8],rax
  5b714b:	49 8b 40 18                                     	mov    rax,QWORD PTR [r8+0x18]
  5b714f:	48 89 81 c8 00 00 00                            	mov    QWORD PTR [rcx+0xc8],rax
  5b7156:	0f 11 81 f0 00 00 00                            	movups XMMWORD PTR [rcx+0xf0],xmm0
  5b715d:	48 89 91 b0 00 00 00                            	mov    QWORD PTR [rcx+0xb0],rdx
  5b7164:	41 8b 10                                        	mov    edx,DWORD PTR [r8]
  5b7167:	f2 41 0f 10 40 48                               	movsd  xmm0,QWORD PTR [r8+0x48]
  5b716d:	0f 11 89 00 01 00 00                            	movups XMMWORD PTR [rcx+0x100],xmm1
  5b7174:	89 91 d0 00 00 00                               	mov    DWORD PTR [rcx+0xd0],edx
  5b717a:	8d 42 fe                                        	lea    eax,[rdx-0x2]
  5b717d:	f2 0f 11 81 10 01 00 00                         	movsd  QWORD PTR [rcx+0x110],xmm0
  5b7185:	83 f8 01                                        	cmp    eax,0x1
  5b7188:	c6 81 24 0e 00 00 00                            	mov    BYTE PTR [rcx+0xe24],0x0
  5b718f:	0f 96 c0                                        	setbe  al
  5b7192:	85 d2                                           	test   edx,edx
  5b7194:	88 81 25 0e 00 00                               	mov    BYTE PTR [rcx+0xe25],al
  5b719a:	0f 94 c0                                        	sete   al
  5b719d:	88 81 26 0e 00 00                               	mov    BYTE PTR [rcx+0xe26],al
  5b71a3:	41 8a 40 24                                     	mov    al,BYTE PTR [r8+0x24]
  5b71a7:	88 81 27 0e 00 00                               	mov    BYTE PTR [rcx+0xe27],al
  5b71ad:	48 81 c1 30 0e 00 00                            	add    rcx,0xe30
  5b71b4:	e8 63 01 00 00                                  	call   0x5b731c
  5b71b9:	80 bb 27 0e 00 00 00                            	cmp    BYTE PTR [rbx+0xe27],0x0
  5b71c0:	48 8d 8b dc 0d 00 00                            	lea    rcx,[rbx+0xddc]
  5b71c7:	ba 20 00 00 00                                  	mov    edx,0x20
  5b71cc:	0f 85 0c 58 42 01                               	jne    0x19dc9de
  5b71d2:	41 8b 78 20                                     	mov    edi,DWORD PTR [r8+0x20]
  5b71d6:	44 8b cf                                        	mov    r9d,edi
  5b71d9:	4c 8d 05 a8 98 33 03                            	lea    r8,[rip+0x33398a8]        # 0x38f0a88
  5b71e0:	e8 cf d7 d5 ff                                  	call   0x3149b4
  5b71e5:	48 8d 05 7c 98 33 03                            	lea    rax,[rip+0x333987c]        # 0x38f0a68
  5b71ec:	89 bb 1c 0e 00 00                               	mov    DWORD PTR [rbx+0xe1c],edi
  5b71f2:	48 89 83 c0 00 00 00                            	mov    QWORD PTR [rbx+0xc0],rax
  5b71f9:	48 8b cb                                        	mov    rcx,rbx
  5b71fc:	48 8b 03                                        	mov    rax,QWORD PTR [rbx]
  5b71ff:	48 8b 5c 24 30                                  	mov    rbx,QWORD PTR [rsp+0x30]
  5b7204:	48 83 c4 20                                     	add    rsp,0x20
  5b7208:	5f                                              	pop    rdi
  5b7209:	48 ff 60 18                                     	rex.W jmp QWORD PTR [rax+0x18]
