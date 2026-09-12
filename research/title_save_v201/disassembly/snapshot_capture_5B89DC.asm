
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b89dc <.data>:
  5b89dc:	48 89 5c 24 08                                  	mov    QWORD PTR [rsp+0x8],rbx
  5b89e1:	48 89 74 24 10                                  	mov    QWORD PTR [rsp+0x10],rsi
  5b89e6:	57                                              	push   rdi
  5b89e7:	48 83 ec 20                                     	sub    rsp,0x20
  5b89eb:	48 8b f1                                        	mov    rsi,rcx
  5b89ee:	48 8d 0d 8b dd 59 04                            	lea    rcx,[rip+0x459dd8b]        # 0x4b56780
  5b89f5:	e8 4a 01 a9 ff                                  	call   0x48b44
  5b89fa:	48 8d 7e 08                                     	lea    rdi,[rsi+0x8]
  5b89fe:	33 d2                                           	xor    edx,edx
  5b8a00:	48 8b cf                                        	mov    rcx,rdi
  5b8a03:	41 b8 00 00 90 00                               	mov    r8d,0x900000
  5b8a09:	e8 c2 b3 5d 00                                  	call   0xb93dd0
  5b8a0e:	48 8b 05 fb 4d 19 04                            	mov    rax,QWORD PTR [rip+0x4194dfb]        # 0x474d810
  5b8a15:	48 8d 9e 08 00 90 00                            	lea    rbx,[rsi+0x900008]
  5b8a1c:	48 8b d3                                        	mov    rdx,rbx
  5b8a1f:	48 8b cf                                        	mov    rcx,rdi
  5b8a22:	4c 8b 00                                        	mov    r8,QWORD PTR [rax]
  5b8a25:	41 c6 80 e4 69 00 00 00                         	mov    BYTE PTR [r8+0x69e4],0x0
  5b8a2d:	e8 9a 00 00 00                                  	call   0x5b8acc
  5b8a32:	e8 b9 ee 61 00                                  	call   0xbd78f0
  5b8a37:	44 8b c0                                        	mov    r8d,eax
  5b8a3a:	89 03                                           	mov    DWORD PTR [rbx],eax
  5b8a3c:	ba 00 00 90 00                                  	mov    edx,0x900000
  5b8a41:	48 8b cf                                        	mov    rcx,rdi
  5b8a44:	e8 23 00 00 00                                  	call   0x5b8a6c
  5b8a49:	48 8d 0d 30 dd 59 04                            	lea    rcx,[rip+0x459dd30]        # 0x4b56780
  5b8a50:	89 86 0c 00 90 00                               	mov    DWORD PTR [rsi+0x90000c],eax
  5b8a56:	48 8b 5c 24 30                                  	mov    rbx,QWORD PTR [rsp+0x30]
  5b8a5b:	48 8b 74 24 38                                  	mov    rsi,QWORD PTR [rsp+0x38]
  5b8a60:	48 83 c4 20                                     	add    rsp,0x20
  5b8a64:	5f                                              	pop    rdi
  5b8a65:	e9 8a 89 5a 00                                  	jmp    0xb613f4
