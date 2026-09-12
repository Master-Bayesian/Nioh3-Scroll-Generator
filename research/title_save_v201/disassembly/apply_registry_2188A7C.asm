
supplied-text-section: file format binary


Disassembly of section .data:

0000000002188a7c <.data>:
 2188a7c:	4c 8b dc                                        	mov    r11,rsp
 2188a7f:	48 81 ec 98 00 00 00                            	sub    rsp,0x98
 2188a86:	49 89 53 90                                     	mov    QWORD PTR [r11-0x70],rdx
 2188a8a:	0f 57 c0                                        	xorps  xmm0,xmm0
 2188a8d:	49 89 4b 88                                     	mov    QWORD PTR [r11-0x78],rcx
 2188a91:	33 d2                                           	xor    edx,edx
 2188a93:	66 0f 7f 44 24 30                               	movdqa XMMWORD PTR [rsp+0x30],xmm0
 2188a99:	33 c0                                           	xor    eax,eax
 2188a9b:	49 89 53 a8                                     	mov    QWORD PTR [r11-0x58],rdx
 2188a9f:	88 54 24 48                                     	mov    BYTE PTR [rsp+0x48],dl
 2188aa3:	49 89 4b b8                                     	mov    QWORD PTR [r11-0x48],rcx
 2188aa7:	49 8d 4b 88                                     	lea    rcx,[r11-0x78]
 2188aab:	88 54 24 70                                     	mov    BYTE PTR [rsp+0x70],dl
 2188aaf:	89 44 24 78                                     	mov    DWORD PTR [rsp+0x78],eax
 2188ab3:	41 88 53 ec                                     	mov    BYTE PTR [r11-0x14],dl
 2188ab7:	e8 74 2c 85 fe                                  	call   0x9db730
 2188abc:	48 8d 4c 24 20                                  	lea    rcx,[rsp+0x20]
 2188ac1:	e8 9e 00 43 fe                                  	call   0x5b8b64
 2188ac6:	48 81 c4 98 00 00 00                            	add    rsp,0x98
 2188acd:	c3                                              	ret
