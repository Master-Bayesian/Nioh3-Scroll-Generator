
supplied-text-section: file format binary


Disassembly of section .data:

00000000005b7050 <.data>:
  5b7050:	40 53                                           	rex push rbx
  5b7052:	48 81 ec 80 00 00 00                            	sub    rsp,0x80
  5b7059:	48 83 3d 1f be 00 04 00                         	cmp    QWORD PTR [rip+0x400be1f],0x0        # 0x45c2e80
  5b7061:	48 8b d9                                        	mov    rbx,rcx
  5b7064:	0f 85 9c 00 00 00                               	jne    0x5b7106
  5b706a:	8b 11                                           	mov    edx,DWORD PTR [rcx]
  5b706c:	b9 fd ff ff ff                                  	mov    ecx,0xfffffffd
  5b7071:	85 d1                                           	test   ecx,edx
  5b7073:	74 2c                                           	je     0x5b70a1
  5b7075:	8d 42 ff                                        	lea    eax,[rdx-0x1]
  5b7078:	85 c1                                           	test   ecx,eax
  5b707a:	74 1c                                           	je     0x5b7098
  5b707c:	83 fa 04                                        	cmp    edx,0x4
  5b707f:	75 09                                           	jne    0x5b708a
  5b7081:	48 8d 0d 48 cf 16 04                            	lea    rcx,[rip+0x416cf48]        # 0x4723fd0
  5b7088:	eb 1e                                           	jmp    0x5b70a8
  5b708a:	83 fa 05                                        	cmp    edx,0x5
  5b708d:	75 77                                           	jne    0x5b7106
  5b708f:	48 8d 0d 3a b8 17 04                            	lea    rcx,[rip+0x417b83a]        # 0x47328d0
  5b7096:	eb 10                                           	jmp    0x5b70a8
  5b7098:	48 8d 0d 31 e6 15 04                            	lea    rcx,[rip+0x415e631]        # 0x47156d0
  5b709f:	eb 07                                           	jmp    0x5b70a8
  5b70a1:	48 8d 0d 18 fd 14 04                            	lea    rcx,[rip+0x414fd18]        # 0x4706dc0
  5b70a8:	48 8b 01                                        	mov    rax,QWORD PTR [rcx]
  5b70ab:	48 89 0d ce bd 00 04                            	mov    QWORD PTR [rip+0x400bdce],rcx        # 0x45c2e80
  5b70b2:	ff 50 08                                        	call   QWORD PTR [rax+0x8]
  5b70b5:	e8 5e 01 00 00                                  	call   0x5b7218
  5b70ba:	0f 28 03                                        	movaps xmm0,XMMWORD PTR [rbx]
  5b70bd:	4c 8d 44 24 20                                  	lea    r8,[rsp+0x20]
  5b70c2:	0f 28 4b 10                                     	movaps xmm1,XMMWORD PTR [rbx+0x10]
  5b70c6:	48 8b d0                                        	mov    rdx,rax
  5b70c9:	48 8b 0d b0 bd 00 04                            	mov    rcx,QWORD PTR [rip+0x400bdb0]        # 0x45c2e80
  5b70d0:	0f 29 44 24 20                                  	movaps XMMWORD PTR [rsp+0x20],xmm0
  5b70d5:	0f 28 43 20                                     	movaps xmm0,XMMWORD PTR [rbx+0x20]
  5b70d9:	0f 29 4c 24 30                                  	movaps XMMWORD PTR [rsp+0x30],xmm1
  5b70de:	0f 28 4b 30                                     	movaps xmm1,XMMWORD PTR [rbx+0x30]
  5b70e2:	48 8b 01                                        	mov    rax,QWORD PTR [rcx]
  5b70e5:	0f 29 44 24 40                                  	movaps XMMWORD PTR [rsp+0x40],xmm0
  5b70ea:	0f 28 43 40                                     	movaps xmm0,XMMWORD PTR [rbx+0x40]
  5b70ee:	0f 29 4c 24 50                                  	movaps XMMWORD PTR [rsp+0x50],xmm1
  5b70f3:	f2 0f 10 4b 50                                  	movsd  xmm1,QWORD PTR [rbx+0x50]
  5b70f8:	0f 29 44 24 60                                  	movaps XMMWORD PTR [rsp+0x60],xmm0
  5b70fd:	f2 0f 11 4c 24 70                               	movsd  QWORD PTR [rsp+0x70],xmm1
  5b7103:	ff 50 20                                        	call   QWORD PTR [rax+0x20]
  5b7106:	48 81 c4 80 00 00 00                            	add    rsp,0x80
  5b710d:	5b                                              	pop    rbx
  5b710e:	c3                                              	ret
