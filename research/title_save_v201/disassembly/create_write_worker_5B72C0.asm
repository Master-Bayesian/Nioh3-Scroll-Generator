
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b72c0 <.data>:
  5b72c0:	40 53                                           	rex push rbx
  5b72c2:	48 83 ec 40                                     	sub    rsp,0x40
  5b72c6:	48 8d 05 c3 f8 4d 03                            	lea    rax,[rip+0x34df8c3]        # 0x3a96b90
  5b72cd:	48 8b d9                                        	mov    rbx,rcx
  5b72d0:	48 89 44 24 30                                  	mov    QWORD PTR [rsp+0x30],rax
  5b72d5:	48 8d 15 14 f3 ff ff                            	lea    rdx,[rip+0xfffffffffffff314]        # 0x5b65f0
  5b72dc:	83 64 24 28 00                                  	and    DWORD PTR [rsp+0x28],0x0
  5b72e1:	4c 8b c1                                        	mov    r8,rcx
  5b72e4:	48 8b 0d 5d f4 ff 03                            	mov    rcx,QWORD PTR [rip+0x3fff45d]        # 0x45b6748
  5b72eb:	41 b9 00 80 00 00                               	mov    r9d,0x8000
  5b72f1:	c7 44 24 20 04 00 00 00                         	mov    DWORD PTR [rsp+0x20],0x4
  5b72f9:	e8 66 00 00 00                                  	call   0x5b7364
  5b72fe:	48 89 43 08                                     	mov    QWORD PTR [rbx+0x8],rax
  5b7302:	48 83 c4 40                                     	add    rsp,0x40
  5b7306:	5b                                              	pop    rbx
  5b7307:	c3                                              	ret
