
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b7518 <.data>:
  5b7518:	40 53                                           	rex push rbx
  5b751a:	48 83 ec 20                                     	sub    rsp,0x20
  5b751e:	45 33 c9                                        	xor    r9d,r9d
  5b7521:	48 8b d9                                        	mov    rbx,rcx
  5b7524:	44 38 49 39                                     	cmp    BYTE PTR [rcx+0x39],r9b
  5b7528:	74 2c                                           	je     0x5b7556
  5b752a:	4c 39 49 08                                     	cmp    QWORD PTR [rcx+0x8],r9
  5b752e:	74 5c                                           	je     0x5b758c
  5b7530:	4c 8b 1d 11 cf 00 04                            	mov    r11,QWORD PTR [rip+0x400cf11]        # 0x45c4448
  5b7537:	49 8b cb                                        	mov    rcx,r11
  5b753a:	e8 81 03 00 00                                  	call   0x5b78c0
  5b753f:	48 85 c0                                        	test   rax,rax
  5b7542:	74 48                                           	je     0x5b758c
  5b7544:	48 8b 50 38                                     	mov    rdx,QWORD PTR [rax+0x38]
  5b7548:	48 8b 4b 08                                     	mov    rcx,QWORD PTR [rbx+0x8]
  5b754c:	e8 23 22 3c 02                                  	call   0x2979774
  5b7551:	49 8b cb                                        	mov    rcx,r11
  5b7554:	eb 31                                           	jmp    0x5b7587
  5b7556:	4c 39 09                                        	cmp    QWORD PTR [rcx],r9
  5b7559:	74 31                                           	je     0x5b758c
  5b755b:	48 8b 0d e6 ce 00 04                            	mov    rcx,QWORD PTR [rip+0x400cee6]        # 0x45c4448
  5b7562:	e8 59 03 00 00                                  	call   0x5b78c0
  5b7567:	48 85 c0                                        	test   rax,rax
  5b756a:	74 20                                           	je     0x5b758c
  5b756c:	48 8b 50 38                                     	mov    rdx,QWORD PTR [rax+0x38]
  5b7570:	48 8b 0b                                        	mov    rcx,QWORD PTR [rbx]
  5b7573:	e8 ec 03 00 00                                  	call   0x5b7964
  5b7578:	48 8b 03                                        	mov    rax,QWORD PTR [rbx]
  5b757b:	8b 08                                           	mov    ecx,DWORD PTR [rax]
  5b757d:	89 4b 14                                        	mov    DWORD PTR [rbx+0x14],ecx
  5b7580:	48 8b 0d c1 ce 00 04                            	mov    rcx,QWORD PTR [rip+0x400cec1]        # 0x45c4448
  5b7587:	e8 40 02 00 00                                  	call   0x5b77cc
  5b758c:	48 83 c4 20                                     	add    rsp,0x20
  5b7590:	5b                                              	pop    rbx
  5b7591:	c3                                              	ret
