
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b6f10 <.data>:
  5b6f10:	48 83 ec 28                                     	sub    rsp,0x28
  5b6f14:	33 c0                                           	xor    eax,eax
  5b6f16:	38 41 38                                        	cmp    BYTE PTR [rcx+0x38],al
  5b6f19:	75 42                                           	jne    0x5b6f5d
  5b6f1b:	88 41 3a                                        	mov    BYTE PTR [rcx+0x3a],al
  5b6f1e:	66 89 41 3c                                     	mov    WORD PTR [rcx+0x3c],ax
  5b6f22:	66 89 41 3f                                     	mov    WORD PTR [rcx+0x3f],ax
  5b6f26:	8b 44 24 60                                     	mov    eax,DWORD PTR [rsp+0x60]
  5b6f2a:	89 41 34                                        	mov    DWORD PTR [rcx+0x34],eax
  5b6f2d:	48 8b 44 24 50                                  	mov    rax,QWORD PTR [rsp+0x50]
  5b6f32:	44 88 49 39                                     	mov    BYTE PTR [rcx+0x39],r9b
  5b6f36:	44 8a 4c 24 58                                  	mov    r9b,BYTE PTR [rsp+0x58]
  5b6f3b:	c6 41 38 01                                     	mov    BYTE PTR [rcx+0x38],0x1
  5b6f3f:	0f 10 00                                        	movups xmm0,XMMWORD PTR [rax]
  5b6f42:	0f 11 41 48                                     	movups XMMWORD PTR [rcx+0x48],xmm0
  5b6f46:	0f 10 48 10                                     	movups xmm1,XMMWORD PTR [rax+0x10]
  5b6f4a:	0f 11 49 58                                     	movups XMMWORD PTR [rcx+0x58],xmm1
  5b6f4e:	f2 0f 10 40 20                                  	movsd  xmm0,QWORD PTR [rax+0x20]
  5b6f53:	f2 0f 11 41 68                                  	movsd  QWORD PTR [rcx+0x68],xmm0
  5b6f58:	e8 07 00 00 00                                  	call   0x5b6f64
  5b6f5d:	48 83 c4 28                                     	add    rsp,0x28
  5b6f61:	c3                                              	ret
