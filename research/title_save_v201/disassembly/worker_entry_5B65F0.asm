
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b65f0 <.data>:
  5b65f0:	48 83 ec 28                                     	sub    rsp,0x28
  5b65f4:	48 8b 0d 85 c8 00 04                            	mov    rcx,QWORD PTR [rip+0x400c885]        # 0x45c2e80
  5b65fb:	48 85 c9                                        	test   rcx,rcx
  5b65fe:	74 06                                           	je     0x5b6606
  5b6600:	48 8b 01                                        	mov    rax,QWORD PTR [rcx]
  5b6603:	ff 50 30                                        	call   QWORD PTR [rax+0x30]
  5b6606:	33 c0                                           	xor    eax,eax
  5b6608:	48 83 c4 28                                     	add    rsp,0x28
  5b660c:	c3                                              	ret
