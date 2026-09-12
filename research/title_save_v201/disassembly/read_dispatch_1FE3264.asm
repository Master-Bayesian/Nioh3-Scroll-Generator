
supplied-text-section: file format binary


Disassembly of section .data:

0000000001fe3264 <.data>:
 1fe3264:	48 89 5c 24 08                                  	mov    QWORD PTR [rsp+0x8],rbx
 1fe3269:	48 89 6c 24 10                                  	mov    QWORD PTR [rsp+0x10],rbp
 1fe326e:	57                                              	push   rdi
 1fe326f:	48 83 ec 20                                     	sub    rsp,0x20
 1fe3273:	83 b9 fc e8 00 00 10                            	cmp    DWORD PTR [rcx+0xe8fc],0x10
 1fe327a:	48 8b d9                                        	mov    rbx,rcx
 1fe327d:	0f 84 9e 00 00 00                               	je     0x1fe3321
 1fe3283:	bd 0f 00 00 00                                  	mov    ebp,0xf
 1fe3288:	48 8d 4b 10                                     	lea    rcx,[rbx+0x10]
 1fe328c:	e8 17 4a 3f fe                                  	call   0x3d7ca8
 1fe3291:	8b 83 fc e8 00 00                               	mov    eax,DWORD PTR [rbx+0xe8fc]
 1fe3297:	83 f8 02                                        	cmp    eax,0x2
 1fe329a:	74 62                                           	je     0x1fe32fe
 1fe329c:	83 f8 08                                        	cmp    eax,0x8
 1fe329f:	74 1c                                           	je     0x1fe32bd
 1fe32a1:	83 f8 0a                                        	cmp    eax,0xa
 1fe32a4:	75 72                                           	jne    0x1fe3318
 1fe32a6:	48 8d 93 dc 0d 00 00                            	lea    rdx,[rbx+0xddc]
 1fe32ad:	48 8b cb                                        	mov    rcx,rbx
 1fe32b0:	e8 d7 59 fd fe                                  	call   0xfb8c8c
 1fe32b5:	89 ab fc e8 00 00                               	mov    DWORD PTR [rbx+0xe8fc],ebp
 1fe32bb:	eb cb                                           	jmp    0x1fe3288
 1fe32bd:	33 d2                                           	xor    edx,edx
 1fe32bf:	48 8b cb                                        	mov    rcx,rbx
 1fe32c2:	e8 e1 dd ff ff                                  	call   0x1fe10a8
 1fe32c7:	80 bb 25 0e 00 00 00                            	cmp    BYTE PTR [rbx+0xe25],0x0
 1fe32ce:	b8 10 00 00 00                                  	mov    eax,0x10
 1fe32d3:	c7 83 fc e8 00 00 10 00 00 00                   	mov    DWORD PTR [rbx+0xe8fc],0x10
 1fe32dd:	75 39                                           	jne    0x1fe3318
 1fe32df:	8b 8b b8 00 00 00                               	mov    ecx,DWORD PTR [rbx+0xb8]
 1fe32e5:	8d 41 f2                                        	lea    eax,[rcx-0xe]
 1fe32e8:	83 f8 01                                        	cmp    eax,0x1
 1fe32eb:	76 23                                           	jbe    0x1fe3310
 1fe32ed:	83 f9 07                                        	cmp    ecx,0x7
 1fe32f0:	74 1e                                           	je     0x1fe3310
 1fe32f2:	b8 10 00 00 00                                  	mov    eax,0x10
 1fe32f7:	83 f9 03                                        	cmp    ecx,0x3
 1fe32fa:	75 1c                                           	jne    0x1fe3318
 1fe32fc:	eb 12                                           	jmp    0x1fe3310
 1fe32fe:	48 8b cb                                        	mov    rcx,rbx
 1fe3301:	e8 be 33 5d fe                                  	call   0x5b66c4
 1fe3306:	b2 01                                           	mov    dl,0x1
 1fe3308:	48 8b cb                                        	mov    rcx,rbx
 1fe330b:	e8 98 dd ff ff                                  	call   0x1fe10a8
 1fe3310:	8b c5                                           	mov    eax,ebp
 1fe3312:	89 ab fc e8 00 00                               	mov    DWORD PTR [rbx+0xe8fc],ebp
 1fe3318:	83 f8 10                                        	cmp    eax,0x10
 1fe331b:	0f 85 67 ff ff ff                               	jne    0x1fe3288
 1fe3321:	48 8b 5c 24 30                                  	mov    rbx,QWORD PTR [rsp+0x30]
 1fe3326:	48 8b 6c 24 38                                  	mov    rbp,QWORD PTR [rsp+0x38]
 1fe332b:	48 83 c4 20                                     	add    rsp,0x20
 1fe332f:	5f                                              	pop    rdi
 1fe3330:	c3                                              	ret
