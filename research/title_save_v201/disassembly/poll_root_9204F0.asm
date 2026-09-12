
supplied-text-section: file format binary


Disassembly of section .data:

00000000009204f0 <.data>:
  9204f0:	40 53                                           	rex push rbx
  9204f2:	48 83 ec 30                                     	sub    rsp,0x30
  9204f6:	83 79 10 00                                     	cmp    DWORD PTR [rcx+0x10],0x0
  9204fa:	48 8b d9                                        	mov    rbx,rcx
  9204fd:	75 4f                                           	jne    0x92054e
  9204ff:	48 8b 09                                        	mov    rcx,QWORD PTR [rcx]
  920502:	e8 65 26 89 ff                                  	call   0x1b2b6c
  920507:	48 8b 03                                        	mov    rax,QWORD PTR [rbx]
  92050a:	80 78 38 00                                     	cmp    BYTE PTR [rax+0x38],0x0
  92050e:	75 7f                                           	jne    0x92058f
  920510:	80 78 3a 00                                     	cmp    BYTE PTR [rax+0x3a],0x0
  920514:	74 07                                           	je     0x92051d
  920516:	b8 01 00 00 00                                  	mov    eax,0x1
  92051b:	eb 2c                                           	jmp    0x920549
  92051d:	8b 48 18                                        	mov    ecx,DWORD PTR [rax+0x18]
  920520:	83 f9 07                                        	cmp    ecx,0x7
  920523:	75 05                                           	jne    0x92052a
  920525:	8d 41 fc                                        	lea    eax,[rcx-0x4]
  920528:	eb 1f                                           	jmp    0x920549
  92052a:	83 f9 0e                                        	cmp    ecx,0xe
  92052d:	75 07                                           	jne    0x920536
  92052f:	b8 04 00 00 00                                  	mov    eax,0x4
  920534:	eb 13                                           	jmp    0x920549
  920536:	83 f9 0f                                        	cmp    ecx,0xf
  920539:	74 f4                                           	je     0x92052f
  92053b:	b8 02 00 00 00                                  	mov    eax,0x2
  920540:	83 f9 06                                        	cmp    ecx,0x6
  920543:	8d 50 03                                        	lea    edx,[rax+0x3]
  920546:	0f 44 c2                                        	cmove  eax,edx
  920549:	89 43 10                                        	mov    DWORD PTR [rbx+0x10],eax
  92054c:	eb 41                                           	jmp    0x92058f
  92054e:	48 8b 41 20                                     	mov    rax,QWORD PTR [rcx+0x20]
  920552:	48 83 c1 18                                     	add    rcx,0x18
  920556:	48 2b 01                                        	sub    rax,QWORD PTR [rcx]
  920559:	48 c1 f8 03                                     	sar    rax,0x3
  92055d:	48 85 c0                                        	test   rax,rax
  920560:	74 2d                                           	je     0x92058f
  920562:	e8 4d 84 c9 ff                                  	call   0x5b89b4
  920567:	48 8b 08                                        	mov    rcx,QWORD PTR [rax]
  92056a:	48 85 c9                                        	test   rcx,rcx
  92056d:	74 20                                           	je     0x92058f
  92056f:	44 8a 49 08                                     	mov    r9b,BYTE PTR [rcx+0x8]
  920573:	b8 01 00 00 00                                  	mov    eax,0x1
  920578:	44 8b 41 04                                     	mov    r8d,DWORD PTR [rcx+0x4]
  92057c:	8b 11                                           	mov    edx,DWORD PTR [rcx]
  92057e:	48 8b cb                                        	mov    rcx,rbx
  920581:	83 64 24 28 00                                  	and    DWORD PTR [rsp+0x28],0x0
  920586:	88 44 24 20                                     	mov    BYTE PTR [rsp+0x20],al
  92058a:	e8 51 66 c9 ff                                  	call   0x5b6be0
  92058f:	83 7b 10 00                                     	cmp    DWORD PTR [rbx+0x10],0x0
  920593:	0f 94 c0                                        	sete   al
  920596:	48 83 c4 30                                     	add    rsp,0x30
  92059a:	5b                                              	pop    rbx
  92059b:	c3                                              	ret
