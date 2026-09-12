
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b65a0 <.data>:
  5b65a0:	48 89 5c 24 08                                  	mov    QWORD PTR [rsp+0x8],rbx
  5b65a5:	57                                              	push   rdi
  5b65a6:	48 83 ec 20                                     	sub    rsp,0x20
  5b65aa:	48 8b 59 28                                     	mov    rbx,QWORD PTR [rcx+0x28]
  5b65ae:	48 8b f9                                        	mov    rdi,rcx
  5b65b1:	e8 c2 02 ca ff                                  	call   0x256878
  5b65b6:	48 8b 4f 20                                     	mov    rcx,QWORD PTR [rdi+0x20]
  5b65ba:	83 ca ff                                        	or     edx,0xffffffff
  5b65bd:	ff 15 7d 7d 32 03                               	call   QWORD PTR [rip+0x3327d7d]        # 0x38de340
  5b65c3:	48 8b 47 30                                     	mov    rax,QWORD PTR [rdi+0x30]
  5b65c7:	48 8b d3                                        	mov    rdx,rbx
  5b65ca:	48 8b cf                                        	mov    rcx,rdi
  5b65cd:	ff d0                                           	call   rax
  5b65cf:	48 8b cf                                        	mov    rcx,rdi
  5b65d2:	8b d8                                           	mov    ebx,eax
  5b65d4:	e8 47 02 ca ff                                  	call   0x256820
  5b65d9:	8b cb                                           	mov    ecx,ebx
  5b65db:	e8 b4 54 62 00                                  	call   0xbdba94
  5b65e0:	8b c3                                           	mov    eax,ebx
  5b65e2:	48 8b 5c 24 30                                  	mov    rbx,QWORD PTR [rsp+0x30]
  5b65e7:	48 83 c4 20                                     	add    rsp,0x20
  5b65eb:	5f                                              	pop    rdi
  5b65ec:	c3                                              	ret
