
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b77cc <.data>:
  5b77cc:	48 89 5c 24 08                                  	mov    QWORD PTR [rsp+0x8],rbx
  5b77d1:	48 89 74 24 10                                  	mov    QWORD PTR [rsp+0x10],rsi
  5b77d6:	57                                              	push   rdi
  5b77d7:	48 83 ec 20                                     	sub    rsp,0x20
  5b77db:	48 8b 59 18                                     	mov    rbx,QWORD PTR [rcx+0x18]
  5b77df:	48 8b f9                                        	mov    rdi,rcx
  5b77e2:	48 8b 41 20                                     	mov    rax,QWORD PTR [rcx+0x20]
  5b77e6:	48 2b c3                                        	sub    rax,rbx
  5b77e9:	48 c1 f8 03                                     	sar    rax,0x3
  5b77ed:	48 85 c0                                        	test   rax,rax
  5b77f0:	74 45                                           	je     0x5b7837
  5b77f2:	48 8b 33                                        	mov    rsi,QWORD PTR [rbx]
  5b77f5:	48 85 f6                                        	test   rsi,rsi
  5b77f8:	74 3d                                           	je     0x5b7837
  5b77fa:	e8 a9 00 00 00                                  	call   0x5b78a8
  5b77ff:	48 8b 56 38                                     	mov    rdx,QWORD PTR [rsi+0x38]
  5b7803:	48 8b c8                                        	mov    rcx,rax
  5b7806:	4c 8b 00                                        	mov    r8,QWORD PTR [rax]
  5b7809:	41 ff 50 58                                     	call   QWORD PTR [r8+0x58]
  5b780d:	e8 96 00 00 00                                  	call   0x5b78a8
  5b7812:	48 8b d6                                        	mov    rdx,rsi
  5b7815:	48 8b c8                                        	mov    rcx,rax
  5b7818:	4c 8b 00                                        	mov    r8,QWORD PTR [rax]
  5b781b:	41 ff 50 58                                     	call   QWORD PTR [r8+0x58]
  5b781f:	4c 8b 47 20                                     	mov    r8,QWORD PTR [rdi+0x20]
  5b7823:	48 8d 53 08                                     	lea    rdx,[rbx+0x8]
  5b7827:	4c 2b c2                                        	sub    r8,rdx
  5b782a:	48 8b cb                                        	mov    rcx,rbx
  5b782d:	e8 ee c8 5d 00                                  	call   0xb94120
  5b7832:	48 83 47 20 f8                                  	add    QWORD PTR [rdi+0x20],0xfffffffffffffff8
  5b7837:	48 8b 5c 24 30                                  	mov    rbx,QWORD PTR [rsp+0x30]
  5b783c:	48 8b 74 24 38                                  	mov    rsi,QWORD PTR [rsp+0x38]
  5b7841:	48 83 c4 20                                     	add    rsp,0x20
  5b7845:	5f                                              	pop    rdi
  5b7846:	c3                                              	ret
