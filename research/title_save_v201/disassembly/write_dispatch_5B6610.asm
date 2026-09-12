
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b6610 <.data>:
  5b6610:	48 89 5c 24 08                                  	mov    QWORD PTR [rsp+0x8],rbx
  5b6615:	57                                              	push   rdi
  5b6616:	48 83 ec 20                                     	sub    rsp,0x20
  5b661a:	83 b9 fc e8 00 00 0d                            	cmp    DWORD PTR [rcx+0xe8fc],0xd
  5b6621:	48 8b d9                                        	mov    rbx,rcx
  5b6624:	0f 84 8e 00 00 00                               	je     0x5b66b8
  5b662a:	48 8d 4b 10                                     	lea    rcx,[rbx+0x10]
  5b662e:	e8 75 16 e2 ff                                  	call   0x3d7ca8
  5b6633:	8b 83 fc e8 00 00                               	mov    eax,DWORD PTR [rbx+0xe8fc]
  5b6639:	83 f8 02                                        	cmp    eax,0x2
  5b663c:	74 4c                                           	je     0x5b668a
  5b663e:	83 f8 06                                        	cmp    eax,0x6
  5b6641:	74 25                                           	je     0x5b6668
  5b6643:	83 f8 08                                        	cmp    eax,0x8
  5b6646:	75 1b                                           	jne    0x5b6663
  5b6648:	48 8d 93 dc 0d 00 00                            	lea    rdx,[rbx+0xddc]
  5b664f:	48 8b cb                                        	mov    rcx,rbx
  5b6652:	e8 35 26 a0 00                                  	call   0xfb8c8c
  5b6657:	c7 83 fc e8 00 00 0c 00 00 00                   	mov    DWORD PTR [rbx+0xe8fc],0xc
  5b6661:	eb c7                                           	jmp    0x5b662a
  5b6663:	83 f8 0d                                        	cmp    eax,0xd
  5b6666:	eb bc                                           	jmp    0x5b6624
  5b6668:	80 bb 24 0e 00 00 00                            	cmp    BYTE PTR [rbx+0xe24],0x0
  5b666f:	48 8b cb                                        	mov    rcx,rbx
  5b6672:	75 33                                           	jne    0x5b66a7
  5b6674:	b2 01                                           	mov    dl,0x1
  5b6676:	e8 51 05 00 00                                  	call   0x5b6bcc
  5b667b:	84 c0                                           	test   al,al
  5b667d:	75 2f                                           	jne    0x5b66ae
  5b667f:	83 bb b8 00 00 00 07                            	cmp    DWORD PTR [rbx+0xb8],0x7
  5b6686:	75 26                                           	jne    0x5b66ae
  5b6688:	eb cd                                           	jmp    0x5b6657
  5b668a:	48 8b cb                                        	mov    rcx,rbx
  5b668d:	e8 32 00 00 00                                  	call   0x5b66c4
  5b6692:	80 bb 26 0e 00 00 00                            	cmp    BYTE PTR [rbx+0xe26],0x0
  5b6699:	74 bc                                           	je     0x5b6657
  5b669b:	b2 01                                           	mov    dl,0x1
  5b669d:	48 8b cb                                        	mov    rcx,rbx
  5b66a0:	e8 03 aa a2 01                                  	call   0x1fe10a8
  5b66a5:	eb b0                                           	jmp    0x5b6657
  5b66a7:	33 d2                                           	xor    edx,edx
  5b66a9:	e8 1e 05 00 00                                  	call   0x5b6bcc
  5b66ae:	c7 83 fc e8 00 00 0d 00 00 00                   	mov    DWORD PTR [rbx+0xe8fc],0xd
  5b66b8:	48 8b 5c 24 30                                  	mov    rbx,QWORD PTR [rsp+0x30]
  5b66bd:	48 83 c4 20                                     	add    rsp,0x20
  5b66c1:	5f                                              	pop    rdi
  5b66c2:	c3                                              	ret
