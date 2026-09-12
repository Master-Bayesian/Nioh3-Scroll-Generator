
supplied-text-section: file format binary


Disassembly of section .data:

00000000009db730 <.data>:
  9db730:	48 89 5c 24 08                                  	mov    QWORD PTR [rsp+0x8],rbx
  9db735:	57                                              	push   rdi
  9db736:	48 83 ec 20                                     	sub    rsp,0x20
  9db73a:	48 8b 1d 7f 14 1f 04                            	mov    rbx,QWORD PTR [rip+0x41f147f]        # 0x4bccbc0
  9db741:	48 8b f9                                        	mov    rdi,rcx
  9db744:	48 85 db                                        	test   rbx,rbx
  9db747:	74 12                                           	je     0x9db75b
  9db749:	48 8b 03                                        	mov    rax,QWORD PTR [rbx]
  9db74c:	48 8b d7                                        	mov    rdx,rdi
  9db74f:	48 8b cb                                        	mov    rcx,rbx
  9db752:	ff 50 08                                        	call   QWORD PTR [rax+0x8]
  9db755:	48 8b 5b 08                                     	mov    rbx,QWORD PTR [rbx+0x8]
  9db759:	eb e9                                           	jmp    0x9db744
  9db75b:	48 8b 5c 24 30                                  	mov    rbx,QWORD PTR [rsp+0x30]
  9db760:	48 83 c4 20                                     	add    rsp,0x20
  9db764:	5f                                              	pop    rdi
  9db765:	c3                                              	ret
