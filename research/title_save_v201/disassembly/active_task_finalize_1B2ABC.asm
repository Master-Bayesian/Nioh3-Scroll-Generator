
supplied-text-section: file format binary


Disassembly of section .data:

00000000001b2abc <.data>:
  1b2abc:	48 83 ec 28                                     	sub    rsp,0x28
  1b2ac0:	48 8b 0d b9 03 41 04                            	mov    rcx,QWORD PTR [rip+0x44103b9]        # 0x45c2e80
  1b2ac7:	48 85 c9                                        	test   rcx,rcx
  1b2aca:	74 0e                                           	je     0x1b2ada
  1b2acc:	48 8b 01                                        	mov    rax,QWORD PTR [rcx]
  1b2acf:	ff 50 10                                        	call   QWORD PTR [rax+0x10]
  1b2ad2:	48 83 25 a6 03 41 04 00                         	and    QWORD PTR [rip+0x44103a6],0x0        # 0x45c2e80
  1b2ada:	48 83 c4 28                                     	add    rsp,0x28
  1b2ade:	c3                                              	ret
