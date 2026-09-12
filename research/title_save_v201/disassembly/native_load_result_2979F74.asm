
supplied-text-section: file format binary


Disassembly of section .data:

0000000002979f74 <.data>:
 2979f74:	40 53                                           	rex push rbx
 2979f76:	48 83 ec 20                                     	sub    rsp,0x20
 2979f7a:	80 79 39 00                                     	cmp    BYTE PTR [rcx+0x39],0x0
 2979f7e:	48 8b d9                                        	mov    rbx,rcx
 2979f81:	74 1c                                           	je     0x2979f9f
 2979f83:	48 8b 49 08                                     	mov    rcx,QWORD PTR [rcx+0x8]
 2979f87:	48 85 c9                                        	test   rcx,rcx
 2979f8a:	74 35                                           	je     0x2979fc1
 2979f8c:	e8 f7 fe ff ff                                  	call   0x2979e88
 2979f91:	84 c0                                           	test   al,al
 2979f93:	0f 94 c0                                        	sete   al
 2979f96:	88 43 42                                        	mov    BYTE PTR [rbx+0x42],al
 2979f99:	48 83 c4 20                                     	add    rsp,0x20
 2979f9d:	5b                                              	pop    rbx
 2979f9e:	c3                                              	ret
 2979f9f:	48 8b 09                                        	mov    rcx,QWORD PTR [rcx]
 2979fa2:	48 85 c9                                        	test   rcx,rcx
 2979fa5:	74 1a                                           	je     0x2979fc1
 2979fa7:	48 83 c1 30                                     	add    rcx,0x30
 2979fab:	e8 60 f6 9c fe                                  	call   0x1349610
 2979fb0:	84 c0                                           	test   al,al
 2979fb2:	0f 94 c1                                        	sete   cl
 2979fb5:	88 4b 41                                        	mov    BYTE PTR [rbx+0x41],cl
 2979fb8:	84 c0                                           	test   al,al
 2979fba:	74 05                                           	je     0x2979fc1
 2979fbc:	e8 0f e8 65 ff                                  	call   0x1fd87d0
 2979fc1:	48 83 c4 20                                     	add    rsp,0x20
 2979fc5:	5b                                              	pop    rbx
 2979fc6:	c3                                              	ret
