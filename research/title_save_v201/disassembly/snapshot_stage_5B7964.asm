
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b7964 <.data>:
  5b7964:	48 89 5c 24 08                                  	mov    QWORD PTR [rsp+0x8],rbx
  5b7969:	57                                              	push   rdi
  5b796a:	48 83 ec 20                                     	sub    rsp,0x20
  5b796e:	48 8b f9                                        	mov    rdi,rcx
  5b7971:	48 8b da                                        	mov    rbx,rdx
  5b7974:	48 8d 0d 05 ee 59 04                            	lea    rcx,[rip+0x459ee05]        # 0x4b56780
  5b797b:	e8 c4 11 a9 ff                                  	call   0x48b44
  5b7980:	48 8d 4f 30                                     	lea    rcx,[rdi+0x30]
  5b7984:	48 8b d3                                        	mov    rdx,rbx
  5b7987:	e8 30 00 00 00                                  	call   0x5b79bc
  5b798c:	c7 07 00 14 11 25                               	mov    DWORD PTR [rdi],0x25111400
  5b7992:	c7 47 04 00 00 01 02                            	mov    DWORD PTR [rdi+0x4],0x2010000
  5b7999:	e8 ba ee a8 ff                                  	call   0x46858
  5b799e:	0f b6 d0                                        	movzx  edx,al
  5b79a1:	48 8d 0d d8 ed 59 04                            	lea    rcx,[rip+0x459edd8]        # 0x4b56780
  5b79a8:	89 57 08                                        	mov    DWORD PTR [rdi+0x8],edx
  5b79ab:	48 8b 5c 24 30                                  	mov    rbx,QWORD PTR [rsp+0x30]
  5b79b0:	48 83 c4 20                                     	add    rsp,0x20
  5b79b4:	5f                                              	pop    rdi
  5b79b5:	e9 3a 9a 5a 00                                  	jmp    0xb613f4
