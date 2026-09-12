
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b62d8 <.data>:
  5b62d8:	48 89 5c 24 08                                  	mov    QWORD PTR [rsp+0x8],rbx
  5b62dd:	48 89 6c 24 10                                  	mov    QWORD PTR [rsp+0x10],rbp
  5b62e2:	48 89 74 24 18                                  	mov    QWORD PTR [rsp+0x18],rsi
  5b62e7:	57                                              	push   rdi
  5b62e8:	48 83 ec 30                                     	sub    rsp,0x30
  5b62ec:	41 8a e9                                        	mov    bpl,r9b
  5b62ef:	41 8b d8                                        	mov    ebx,r8d
  5b62f2:	8b fa                                           	mov    edi,edx
  5b62f4:	48 8b f1                                        	mov    rsi,rcx
  5b62f7:	e8 28 d6 cc ff                                  	call   0x283924
  5b62fc:	84 c0                                           	test   al,al
  5b62fe:	75 25                                           	jne    0x5b6325
  5b6300:	45 33 c9                                        	xor    r9d,r9d
  5b6303:	48 8b ce                                        	mov    rcx,rsi
  5b6306:	e8 01 13 00 00                                  	call   0x5b760c
  5b630b:	83 64 24 28 00                                  	and    DWORD PTR [rsp+0x28],0x0
  5b6310:	45 33 c9                                        	xor    r9d,r9d
  5b6313:	44 8b c3                                        	mov    r8d,ebx
  5b6316:	40 88 6c 24 20                                  	mov    BYTE PTR [rsp+0x20],bpl
  5b631b:	8b d7                                           	mov    edx,edi
  5b631d:	48 8b ce                                        	mov    rcx,rsi
  5b6320:	e8 bb 08 00 00                                  	call   0x5b6be0
  5b6325:	48 8b 5c 24 40                                  	mov    rbx,QWORD PTR [rsp+0x40]
  5b632a:	48 8b 6c 24 48                                  	mov    rbp,QWORD PTR [rsp+0x48]
  5b632f:	48 8b 74 24 50                                  	mov    rsi,QWORD PTR [rsp+0x50]
  5b6334:	48 83 c4 30                                     	add    rsp,0x30
  5b6338:	5f                                              	pop    rdi
  5b6339:	c3                                              	ret
