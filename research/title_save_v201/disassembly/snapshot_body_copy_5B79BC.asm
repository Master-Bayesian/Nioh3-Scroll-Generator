; Leaf entry: no standalone .pdata interval; disassembled to next known function.

supplied-text-section: file format binary


Disassembly of section .data:

00000000005b79bc <.data>:
  5b79bc:	4c 8b ca                                        	mov    r9,rdx
  5b79bf:	4c 8d 41 08                                     	lea    r8,[rcx+0x8]
  5b79c3:	4c 2b c9                                        	sub    r9,rcx
  5b79c6:	41 ba 00 00 90 00                               	mov    r10d,0x900000
  5b79cc:	43 8a 04 01                                     	mov    al,BYTE PTR [r9+r8*1]
  5b79d0:	41 88 00                                        	mov    BYTE PTR [r8],al
  5b79d3:	49 ff c0                                        	inc    r8
  5b79d6:	49 83 ea 01                                     	sub    r10,0x1
  5b79da:	75 f0                                           	jne    0x5b79cc
  5b79dc:	8b 82 08 00 90 00                               	mov    eax,DWORD PTR [rdx+0x900008]
  5b79e2:	45 8d 42 06                                     	lea    r8d,[r10+0x6]
  5b79e6:	89 81 08 00 90 00                               	mov    DWORD PTR [rcx+0x900008],eax
  5b79ec:	8b 82 0c 00 90 00                               	mov    eax,DWORD PTR [rdx+0x90000c]
  5b79f2:	48 8d 91 10 00 90 00                            	lea    rdx,[rcx+0x900010]
  5b79f9:	89 81 0c 00 90 00                               	mov    DWORD PTR [rcx+0x90000c],eax
  5b79ff:	42 8b 04 0a                                     	mov    eax,DWORD PTR [rdx+r9*1]
  5b7a03:	89 02                                           	mov    DWORD PTR [rdx],eax
  5b7a05:	48 8d 52 04                                     	lea    rdx,[rdx+0x4]
  5b7a09:	49 83 e8 01                                     	sub    r8,0x1
  5b7a0d:	75 f0                                           	jne    0x5b79ff
  5b7a0f:	48 8b c1                                        	mov    rax,rcx
  5b7a12:	c3                                              	ret
  5b7a13:	cc                                              	int3
