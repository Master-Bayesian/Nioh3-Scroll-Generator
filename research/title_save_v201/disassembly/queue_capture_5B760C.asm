
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b760c <.data>:
  5b760c:	f7 c2 fd ff ff ff                               	test   edx,0xfffffffd
  5b7612:	0f 85 91 01 00 00                               	jne    0x5b77a9
  5b7618:	48 89 5c 24 08                                  	mov    QWORD PTR [rsp+0x8],rbx
  5b761d:	48 89 74 24 10                                  	mov    QWORD PTR [rsp+0x10],rsi
  5b7622:	48 89 7c 24 18                                  	mov    QWORD PTR [rsp+0x18],rdi
  5b7627:	55                                              	push   rbp
  5b7628:	41 54                                           	push   r12
  5b762a:	41 55                                           	push   r13
  5b762c:	41 56                                           	push   r14
  5b762e:	41 57                                           	push   r15
  5b7630:	48 8b ec                                        	mov    rbp,rsp
  5b7633:	48 83 ec 60                                     	sub    rsp,0x60
  5b7637:	45 33 e4                                        	xor    r12d,r12d
  5b763a:	4c 8d 71 18                                     	lea    r14,[rcx+0x18]
  5b763e:	45 8b f8                                        	mov    r15d,r8d
  5b7641:	4c 89 65 c0                                     	mov    QWORD PTR [rbp-0x40],r12
  5b7645:	4d 8b 46 08                                     	mov    r8,QWORD PTR [r14+0x8]
  5b7649:	48 8b f1                                        	mov    rsi,rcx
  5b764c:	49 8b 0e                                        	mov    rcx,QWORD PTR [r14]
  5b764f:	41 8a d9                                        	mov    bl,r9b
  5b7652:	8b fa                                           	mov    edi,edx
  5b7654:	45 8b d4                                        	mov    r10d,r12d
  5b7657:	49 3b c8                                        	cmp    rcx,r8
  5b765a:	74 20                                           	je     0x5b767c
  5b765c:	48 8b 01                                        	mov    rax,QWORD PTR [rcx]
  5b765f:	38 58 08                                        	cmp    BYTE PTR [rax+0x8],bl
  5b7662:	4c 0f 44 d0                                     	cmove  r10,rax
  5b7666:	48 83 c1 08                                     	add    rcx,0x8
  5b766a:	4c 89 55 c0                                     	mov    QWORD PTR [rbp-0x40],r10
  5b766e:	49 3b c8                                        	cmp    rcx,r8
  5b7671:	75 e9                                           	jne    0x5b765c
  5b7673:	4d 85 d2                                        	test   r10,r10
  5b7676:	0f 85 d2 00 00 00                               	jne    0x5b774e
  5b767c:	4d 2b 06                                        	sub    r8,QWORD PTR [r14]
  5b767f:	49 c1 f8 03                                     	sar    r8,0x3
  5b7683:	49 83 f8 02                                     	cmp    r8,0x2
  5b7687:	0f 83 ff 00 00 00                               	jae    0x5b778c
  5b768d:	e8 16 02 00 00                                  	call   0x5b78a8
  5b7692:	4c 8d 45 c8                                     	lea    r8,[rbp-0x38]
  5b7696:	ba 40 00 00 00                                  	mov    edx,0x40
  5b769b:	48 8b 08                                        	mov    rcx,QWORD PTR [rax]
  5b769e:	c7 45 c8 35 00 00 00                            	mov    DWORD PTR [rbp-0x38],0x35
  5b76a5:	4c 89 65 d0                                     	mov    QWORD PTR [rbp-0x30],r12
  5b76a9:	4c 8b 49 30                                     	mov    r9,QWORD PTR [rcx+0x30]
  5b76ad:	48 8b c8                                        	mov    rcx,rax
  5b76b0:	41 ff d1                                        	call   r9
  5b76b3:	48 89 45 c0                                     	mov    QWORD PTR [rbp-0x40],rax
  5b76b7:	48 85 c0                                        	test   rax,rax
  5b76ba:	0f 84 cc 00 00 00                               	je     0x5b778c
  5b76c0:	e8 e3 01 00 00                                  	call   0x5b78a8
  5b76c5:	4c 8d 45 c8                                     	lea    r8,[rbp-0x38]
  5b76c9:	48 8b 08                                        	mov    rcx,QWORD PTR [rax]
  5b76cc:	c7 45 c8 35 00 00 00                            	mov    DWORD PTR [rbp-0x38],0x35
  5b76d3:	4c 89 65 d0                                     	mov    QWORD PTR [rbp-0x30],r12
  5b76d7:	84 db                                           	test   bl,bl
  5b76d9:	74 0b                                           	je     0x5b76e6
  5b76db:	4c 8b 49 30                                     	mov    r9,QWORD PTR [rcx+0x30]
  5b76df:	ba 20 98 03 00                                  	mov    edx,0x39820
  5b76e4:	eb 09                                           	jmp    0x5b76ef
  5b76e6:	4c 8b 49 28                                     	mov    r9,QWORD PTR [rcx+0x28]
  5b76ea:	ba 28 00 90 00                                  	mov    edx,0x900028
  5b76ef:	48 8b c8                                        	mov    rcx,rax
  5b76f2:	41 ff d1                                        	call   r9
  5b76f5:	48 8b 4d c0                                     	mov    rcx,QWORD PTR [rbp-0x40]
  5b76f9:	48 89 41 38                                     	mov    QWORD PTR [rcx+0x38],rax
  5b76fd:	48 8b 55 c0                                     	mov    rdx,QWORD PTR [rbp-0x40]
  5b7701:	4c 39 62 38                                     	cmp    QWORD PTR [rdx+0x38],r12
  5b7705:	75 11                                           	jne    0x5b7718
  5b7707:	e8 9c 01 00 00                                  	call   0x5b78a8
  5b770c:	48 8b c8                                        	mov    rcx,rax
  5b770f:	4c 8b 00                                        	mov    r8,QWORD PTR [rax]
  5b7712:	41 ff 50 58                                     	call   QWORD PTR [r8+0x58]
  5b7716:	eb 74                                           	jmp    0x5b778c
  5b7718:	48 8d 4d d8                                     	lea    rcx,[rbp-0x28]
  5b771c:	e8 8f e4 d8 ff                                  	call   0x345bb0
  5b7721:	48 8d 55 c0                                     	lea    rdx,[rbp-0x40]
  5b7725:	49 8b ce                                        	mov    rcx,r14
  5b7728:	0f 10 08                                        	movups xmm1,XMMWORD PTR [rax]
  5b772b:	0f 10 50 10                                     	movups xmm2,XMMWORD PTR [rax+0x10]
  5b772f:	f2 0f 10 40 20                                  	movsd  xmm0,QWORD PTR [rax+0x20]
  5b7734:	48 8b 45 c0                                     	mov    rax,QWORD PTR [rbp-0x40]
  5b7738:	0f 11 48 10                                     	movups XMMWORD PTR [rax+0x10],xmm1
  5b773c:	0f 11 50 20                                     	movups XMMWORD PTR [rax+0x20],xmm2
  5b7740:	f2 0f 11 40 30                                  	movsd  QWORD PTR [rax+0x30],xmm0
  5b7745:	e8 ea 44 4a 00                                  	call   0xa5bc34
  5b774a:	4c 8b 55 c0                                     	mov    r10,QWORD PTR [rbp-0x40]
  5b774e:	41 89 3a                                        	mov    DWORD PTR [r10],edi
  5b7751:	48 8b 45 c0                                     	mov    rax,QWORD PTR [rbp-0x40]
  5b7755:	44 89 78 04                                     	mov    DWORD PTR [rax+0x4],r15d
  5b7759:	48 8b 45 c0                                     	mov    rax,QWORD PTR [rbp-0x40]
  5b775d:	88 58 08                                        	mov    BYTE PTR [rax+0x8],bl
  5b7760:	84 db                                           	test   bl,bl
  5b7762:	74 1b                                           	je     0x5b777f
  5b7764:	48 8b 45 c0                                     	mov    rax,QWORD PTR [rbp-0x40]
  5b7768:	48 8b 58 38                                     	mov    rbx,QWORD PTR [rax+0x38]
  5b776c:	48 8b cb                                        	mov    rcx,rbx
  5b776f:	e8 78 29 3c 02                                  	call   0x297a0ec
  5b7774:	48 8b 0e                                        	mov    rcx,QWORD PTR [rsi]
  5b7777:	8b 43 08                                        	mov    eax,DWORD PTR [rbx+0x8]
  5b777a:	89 41 10                                        	mov    DWORD PTR [rcx+0x10],eax
  5b777d:	eb 0d                                           	jmp    0x5b778c
  5b777f:	48 8b 4d c0                                     	mov    rcx,QWORD PTR [rbp-0x40]
  5b7783:	48 8b 49 38                                     	mov    rcx,QWORD PTR [rcx+0x38]
  5b7787:	e8 50 12 00 00                                  	call   0x5b89dc
  5b778c:	4c 8d 5c 24 60                                  	lea    r11,[rsp+0x60]
  5b7791:	49 8b 5b 30                                     	mov    rbx,QWORD PTR [r11+0x30]
  5b7795:	49 8b 73 38                                     	mov    rsi,QWORD PTR [r11+0x38]
  5b7799:	49 8b 7b 40                                     	mov    rdi,QWORD PTR [r11+0x40]
  5b779d:	49 8b e3                                        	mov    rsp,r11
  5b77a0:	41 5f                                           	pop    r15
  5b77a2:	41 5e                                           	pop    r14
  5b77a4:	41 5d                                           	pop    r13
  5b77a6:	41 5c                                           	pop    r12
  5b77a8:	5d                                              	pop    rbp
  5b77a9:	c3                                              	ret
