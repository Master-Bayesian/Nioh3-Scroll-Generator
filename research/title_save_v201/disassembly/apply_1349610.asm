
supplied-text-section: file format binary


Disassembly of section .data:

0000000001349610 <.data>:
 1349610:	48 89 5c 24 08                                  	mov    QWORD PTR [rsp+0x8],rbx
 1349615:	48 89 74 24 10                                  	mov    QWORD PTR [rsp+0x10],rsi
 134961a:	57                                              	push   rdi
 134961b:	48 83 ec 20                                     	sub    rsp,0x20
 134961f:	4c 8d 99 08 00 90 00                            	lea    r11,[rcx+0x900008]
 1349626:	48 8b f1                                        	mov    rsi,rcx
 1349629:	45 8b 03                                        	mov    r8d,DWORD PTR [r11]
 134962c:	48 83 c1 08                                     	add    rcx,0x8
 1349630:	ba 00 00 90 00                                  	mov    edx,0x900000
 1349635:	e8 32 f4 26 ff                                  	call   0x5b8a6c
 134963a:	49 8b d3                                        	mov    rdx,r11
 134963d:	48 8d 4e 08                                     	lea    rcx,[rsi+0x8]
 1349641:	8b f8                                           	mov    edi,eax
 1349643:	e8 34 f4 e3 00                                  	call   0x2188a7c
 1349648:	48 8b 0d 91 3e 40 03                            	mov    rcx,QWORD PTR [rip+0x3403e91]        # 0x474d4e0
 134964f:	e8 24 9a e3 00                                  	call   0x2183078
 1349654:	48 8b 0d b5 41 40 03                            	mov    rcx,QWORD PTR [rip+0x34041b5]        # 0x474d810
 134965b:	e8 ac 87 e0 00                                  	call   0x2151e0c
 1349660:	48 8b 0d 79 3e 40 03                            	mov    rcx,QWORD PTR [rip+0x3403e79]        # 0x474d4e0
 1349667:	e8 70 86 e0 00                                  	call   0x2151cdc
 134966c:	48 8b 0d 85 41 40 03                            	mov    rcx,QWORD PTR [rip+0x3404185]        # 0x474d7f8
 1349673:	e8 3c 87 e0 00                                  	call   0x2151db4
 1349678:	48 8b 0d e1 3d 40 03                            	mov    rcx,QWORD PTR [rip+0x3403de1]        # 0x474d460
 134967f:	48 8b 09                                        	mov    rcx,QWORD PTR [rcx]
 1349682:	e8 81 e7 e0 00                                  	call   0x2157e08
 1349687:	48 8b 0d 9a 41 40 03                            	mov    rcx,QWORD PTR [rip+0x340419a]        # 0x474d828
 134968e:	e8 41 88 e0 00                                  	call   0x2151ed4
 1349693:	e8 e4 d2 ee 00                                  	call   0x223697c
 1349698:	e8 ff d3 f5 00                                  	call   0x22a6a9c
 134969d:	3b be 0c 00 90 00                               	cmp    edi,DWORD PTR [rsi+0x90000c]
 13496a3:	48 8b 5c 24 30                                  	mov    rbx,QWORD PTR [rsp+0x30]
 13496a8:	48 8b 74 24 38                                  	mov    rsi,QWORD PTR [rsp+0x38]
 13496ad:	0f 94 c0                                        	sete   al
 13496b0:	48 83 c4 20                                     	add    rsp,0x20
 13496b4:	5f                                              	pop    rdi
 13496b5:	c3                                              	ret
