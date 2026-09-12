
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b6be0 <.data>:
  5b6be0:	40 53                                           	rex push rbx
  5b6be2:	48 83 ec 70                                     	sub    rsp,0x70
  5b6be6:	f7 c2 fd ff ff ff                               	test   edx,0xfffffffd
  5b6bec:	48 8b d9                                        	mov    rbx,rcx
  5b6bef:	48 8b 09                                        	mov    rcx,QWORD PTR [rcx]
  5b6bf2:	0f 94 c0                                        	sete   al
  5b6bf5:	80 79 38 00                                     	cmp    BYTE PTR [rcx+0x38],0x0
  5b6bf9:	0f 85 90 00 00 00                               	jne    0x5b6c8f
  5b6bff:	83 63 10 00                                     	and    DWORD PTR [rbx+0x10],0x0
  5b6c03:	84 c0                                           	test   al,al
  5b6c05:	74 4f                                           	je     0x5b6c56
  5b6c07:	48 8d 4b 18                                     	lea    rcx,[rbx+0x18]
  5b6c0b:	48 8b 41 08                                     	mov    rax,QWORD PTR [rcx+0x8]
  5b6c0f:	48 2b 01                                        	sub    rax,QWORD PTR [rcx]
  5b6c12:	48 c1 f8 03                                     	sar    rax,0x3
  5b6c16:	48 85 c0                                        	test   rax,rax
  5b6c19:	74 74                                           	je     0x5b6c8f
  5b6c1b:	e8 94 1d 00 00                                  	call   0x5b89b4
  5b6c20:	48 8b 10                                        	mov    rdx,QWORD PTR [rax]
  5b6c23:	48 85 d2                                        	test   rdx,rdx
  5b6c26:	74 67                                           	je     0x5b6c8f
  5b6c28:	8b 84 24 a8 00 00 00                            	mov    eax,DWORD PTR [rsp+0xa8]
  5b6c2f:	48 8d 4a 10                                     	lea    rcx,[rdx+0x10]
  5b6c33:	44 8a 4a 08                                     	mov    r9b,BYTE PTR [rdx+0x8]
  5b6c37:	44 8b 42 04                                     	mov    r8d,DWORD PTR [rdx+0x4]
  5b6c3b:	8b 12                                           	mov    edx,DWORD PTR [rdx]
  5b6c3d:	89 44 24 30                                     	mov    DWORD PTR [rsp+0x30],eax
  5b6c41:	8a 84 24 a0 00 00 00                            	mov    al,BYTE PTR [rsp+0xa0]
  5b6c48:	88 44 24 28                                     	mov    BYTE PTR [rsp+0x28],al
  5b6c4c:	48 89 4c 24 20                                  	mov    QWORD PTR [rsp+0x20],rcx
  5b6c51:	48 8b 0b                                        	mov    rcx,QWORD PTR [rbx]
  5b6c54:	eb 34                                           	jmp    0x5b6c8a
  5b6c56:	33 c0                                           	xor    eax,eax
  5b6c58:	0f 57 c0                                        	xorps  xmm0,xmm0
  5b6c5b:	48 89 44 24 60                                  	mov    QWORD PTR [rsp+0x60],rax
  5b6c60:	8b 84 24 a8 00 00 00                            	mov    eax,DWORD PTR [rsp+0xa8]
  5b6c67:	89 44 24 30                                     	mov    DWORD PTR [rsp+0x30],eax
  5b6c6b:	8a 84 24 a0 00 00 00                            	mov    al,BYTE PTR [rsp+0xa0]
  5b6c72:	88 44 24 28                                     	mov    BYTE PTR [rsp+0x28],al
  5b6c76:	48 8d 44 24 40                                  	lea    rax,[rsp+0x40]
  5b6c7b:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
  5b6c80:	0f 11 44 24 40                                  	movups XMMWORD PTR [rsp+0x40],xmm0
  5b6c85:	0f 11 44 24 50                                  	movups XMMWORD PTR [rsp+0x50],xmm0
  5b6c8a:	e8 81 02 00 00                                  	call   0x5b6f10
  5b6c8f:	48 83 c4 70                                     	add    rsp,0x70
  5b6c93:	5b                                              	pop    rbx
  5b6c94:	c3                                              	ret
