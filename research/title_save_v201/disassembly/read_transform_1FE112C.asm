
supplied-text-section: file format binary


Disassembly of section .data:

0000000001fe112c <.data>:
 1fe112c:	48 8b c4                                        	mov    rax,rsp
 1fe112f:	48 89 58 10                                     	mov    QWORD PTR [rax+0x10],rbx
 1fe1133:	48 89 70 18                                     	mov    QWORD PTR [rax+0x18],rsi
 1fe1137:	48 89 78 20                                     	mov    QWORD PTR [rax+0x20],rdi
 1fe113b:	55                                              	push   rbp
 1fe113c:	41 54                                           	push   r12
 1fe113e:	41 55                                           	push   r13
 1fe1140:	41 56                                           	push   r14
 1fe1142:	41 57                                           	push   r15
 1fe1144:	48 8d a8 38 fd ff ff                            	lea    rbp,[rax-0x2c8]
 1fe114b:	48 81 ec a0 03 00 00                            	sub    rsp,0x3a0
 1fe1152:	0f 29 70 c8                                     	movaps XMMWORD PTR [rax-0x38],xmm6
 1fe1156:	48 8b 05 d3 2d 4d 02                            	mov    rax,QWORD PTR [rip+0x24d2dd3]        # 0x44b3f30
 1fe115d:	48 33 c4                                        	xor    rax,rsp
 1fe1160:	48 89 85 80 02 00 00                            	mov    QWORD PTR [rbp+0x280],rax
 1fe1167:	48 8b f1                                        	mov    rsi,rcx
 1fe116a:	88 54 24 40                                     	mov    BYTE PTR [rsp+0x40],dl
 1fe116e:	48 81 c1 30 0e 00 00                            	add    rcx,0xe30
 1fe1175:	44 8a f2                                        	mov    r14b,dl
 1fe1178:	e8 9f 61 5d fe                                  	call   0x5b731c
 1fe117d:	45 33 e4                                        	xor    r12d,r12d
 1fe1180:	41 8b dc                                        	mov    ebx,r12d
 1fe1183:	44 89 a6 f0 e8 00 00                            	mov    DWORD PTR [rsi+0xe8f0],r12d
 1fe118a:	89 5c 24 60                                     	mov    DWORD PTR [rsp+0x60],ebx
 1fe118e:	44 88 a6 29 0e 00 00                            	mov    BYTE PTR [rsi+0xe29],r12b
 1fe1195:	44 39 a6 d8 0d 00 00                            	cmp    DWORD PTR [rsi+0xdd8],r12d
 1fe119c:	0f 86 9f 08 00 00                               	jbe    0x1fe1a41
 1fe11a2:	45 84 f6                                        	test   r14b,r14b
 1fe11a5:	75 1a                                           	jne    0x1fe11c1
 1fe11a7:	44 38 a6 27 0e 00 00                            	cmp    BYTE PTR [rsi+0xe27],r12b
 1fe11ae:	75 11                                           	jne    0x1fe11c1
 1fe11b0:	48 8b ce                                        	mov    rcx,rsi
 1fe11b3:	e8 10 22 1d fe                                  	call   0x1b33c8
 1fe11b8:	4c 8d be dc 0d 00 00                            	lea    r15,[rsi+0xddc]
 1fe11bf:	eb 10                                           	jmp    0x1fe11d1
 1fe11c1:	8b c3                                           	mov    eax,ebx
 1fe11c3:	4c 8d be 18 01 00 00                            	lea    r15,[rsi+0x118]
 1fe11ca:	48 c1 e0 06                                     	shl    rax,0x6
 1fe11ce:	4c 03 f8                                        	add    r15,rax
 1fe11d1:	44 8b eb                                        	mov    r13d,ebx
 1fe11d4:	4c 8d a6 b0 00 00 00                            	lea    r12,[rsi+0xb0]
 1fe11db:	4c 8d b6 c0 00 00 00                            	lea    r14,[rsi+0xc0]
 1fe11e2:	49 8b fc                                        	mov    rdi,r12
 1fe11e5:	49 8b 3c 24                                     	mov    rdi,QWORD PTR [r12]
 1fe11e9:	49 8b de                                        	mov    rbx,r14
 1fe11ec:	48 8b 1b                                        	mov    rbx,QWORD PTR [rbx]
 1fe11ef:	e8 dc 68 fd fe                                  	call   0xfb7ad0
 1fe11f4:	48 89 5c 24 30                                  	mov    QWORD PTR [rsp+0x30],rbx
 1fe11f9:	4c 8d 05 20 59 ab 01                            	lea    r8,[rip+0x1ab5920]        # 0x3a96b20
 1fe1200:	4c 8b c8                                        	mov    r9,rax
 1fe1203:	4c 89 7c 24 28                                  	mov    QWORD PTR [rsp+0x28],r15
 1fe1208:	ba 05 01 00 00                                  	mov    edx,0x105
 1fe120d:	48 89 7c 24 20                                  	mov    QWORD PTR [rsp+0x20],rdi
 1fe1212:	48 8d 4d 70                                     	lea    rcx,[rbp+0x70]
 1fe1216:	e8 99 37 33 fe                                  	call   0x3149b4
 1fe121b:	33 db                                           	xor    ebx,ebx
 1fe121d:	38 1d 09 4e 5d 02                               	cmp    BYTE PTR [rip+0x25d4e09],bl        # 0x45b602c
 1fe1223:	75 66                                           	jne    0x1fe128b
 1fe1225:	38 9e 27 0e 00 00                               	cmp    BYTE PTR [rsi+0xe27],bl
 1fe122b:	74 5e                                           	je     0x1fe128b
 1fe122d:	41 8b c5                                        	mov    eax,r13d
 1fe1230:	4c 89 74 24 20                                  	mov    QWORD PTR [rsp+0x20],r14
 1fe1235:	48 c1 e0 06                                     	shl    rax,0x6
 1fe1239:	4c 8d 8e 18 01 00 00                            	lea    r9,[rsi+0x118]
 1fe1240:	4c 03 c8                                        	add    r9,rax
 1fe1243:	4d 8b c4                                        	mov    r8,r12
 1fe1246:	e8 91 23 ff ff                                  	call   0x1fd35dc
 1fe124b:	48 8d 05 fe d9 b8 02                            	lea    rax,[rip+0x2b8d9fe]        # 0x4b6ec50
 1fe1252:	4c 8d 45 70                                     	lea    r8,[rbp+0x70]
 1fe1256:	4c 2b c0                                        	sub    r8,rax
 1fe1259:	0f b7 10                                        	movzx  edx,WORD PTR [rax]
 1fe125c:	42 0f b7 0c 00                                  	movzx  ecx,WORD PTR [rax+r8*1]
 1fe1261:	2b d1                                           	sub    edx,ecx
 1fe1263:	75 08                                           	jne    0x1fe126d
 1fe1265:	48 83 c0 02                                     	add    rax,0x2
 1fe1269:	85 c9                                           	test   ecx,ecx
 1fe126b:	75 ec                                           	jne    0x1fe1259
 1fe126d:	85 d2                                           	test   edx,edx
 1fe126f:	75 1a                                           	jne    0x1fe128b
 1fe1271:	c6 86 28 0e 00 00 01                            	mov    BYTE PTR [rsi+0xe28],0x1
 1fe1278:	c6 05 ad 4d 5d 02 01                            	mov    BYTE PTR [rip+0x25d4dad],0x1        # 0x45b602c
 1fe127f:	ff 15 13 d1 8f 01                               	call   QWORD PTR [rip+0x18fd113]        # 0x38de398
 1fe1285:	89 05 a5 4d 5d 02                               	mov    DWORD PTR [rip+0x25d4da5],eax        # 0x45b6030
 1fe128b:	44 8a 74 24 40                                  	mov    r14b,BYTE PTR [rsp+0x40]
 1fe1290:	45 84 f6                                        	test   r14b,r14b
 1fe1293:	0f 84 74 02 00 00                               	je     0x1fe150d
 1fe1299:	41 bf 58 01 00 00                               	mov    r15d,0x158
 1fe129f:	e8 04 66 5d fe                                  	call   0x5b78a8
 1fe12a4:	4c 8d 45 a0                                     	lea    r8,[rbp-0x60]
 1fe12a8:	c7 45 a0 35 00 00 00                            	mov    DWORD PTR [rbp-0x60],0x35
 1fe12af:	49 8b d7                                        	mov    rdx,r15
 1fe12b2:	48 89 5d a8                                     	mov    QWORD PTR [rbp-0x58],rbx
 1fe12b6:	48 8b f8                                        	mov    rdi,rax
 1fe12b9:	48 8b 08                                        	mov    rcx,QWORD PTR [rax]
 1fe12bc:	4c 8b 49 28                                     	mov    r9,QWORD PTR [rcx+0x28]
 1fe12c0:	48 8b c8                                        	mov    rcx,rax
 1fe12c3:	41 ff d1                                        	call   r9
 1fe12c6:	48 8b d8                                        	mov    rbx,rax
 1fe12c9:	48 85 c0                                        	test   rax,rax
 1fe12cc:	0f 84 32 07 00 00                               	je     0x1fe1a04
 1fe12d2:	4d 8b c7                                        	mov    r8,r15
 1fe12d5:	48 8d 4d 70                                     	lea    rcx,[rbp+0x70]
 1fe12d9:	48 8b d0                                        	mov    rdx,rax
 1fe12dc:	e8 27 51 ff ff                                  	call   0x1fd6408
 1fe12e1:	33 d2                                           	xor    edx,edx
 1fe12e3:	84 c0                                           	test   al,al
 1fe12e5:	0f 84 0a 07 00 00                               	je     0x1fe19f5
 1fe12eb:	89 96 b8 00 00 00                               	mov    DWORD PTR [rsi+0xb8],edx
 1fe12f1:	4c 8d 45 b0                                     	lea    r8,[rbp-0x50]
 1fe12f5:	48 8b 0f                                        	mov    rcx,QWORD PTR [rdi]
 1fe12f8:	48 89 55 b8                                     	mov    QWORD PTR [rbp-0x48],rdx
 1fe12fc:	ba 58 01 00 00                                  	mov    edx,0x158
 1fe1301:	c7 45 b0 35 00 00 00                            	mov    DWORD PTR [rbp-0x50],0x35
 1fe1308:	4c 8b 49 28                                     	mov    r9,QWORD PTR [rcx+0x28]
 1fe130c:	48 8b cf                                        	mov    rcx,rdi
 1fe130f:	41 ff d1                                        	call   r9
 1fe1312:	4c 8b f0                                        	mov    r14,rax
 1fe1315:	48 85 c0                                        	test   rax,rax
 1fe1318:	0f 84 2a 02 00 00                               	je     0x1fe1548
 1fe131e:	4c 8d 4d 30                                     	lea    r9,[rbp+0x30]
 1fe1322:	4c 8d 45 20                                     	lea    r8,[rbp+0x20]
 1fe1326:	48 8d 55 10                                     	lea    rdx,[rbp+0x10]
 1fe132a:	48 8d 4d 00                                     	lea    rcx,[rbp+0x0]
 1fe132e:	e8 0d 7c fd fe                                  	call   0xfb8f40
 1fe1333:	84 c0                                           	test   al,al
 1fe1335:	75 0a                                           	jne    0x1fe1341
 1fe1337:	c7 86 b8 00 00 00 0d 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0xd
 1fe1341:	48 8d 55 20                                     	lea    rdx,[rbp+0x20]
 1fe1345:	48 8d 0d 44 13 b8 02                            	lea    rcx,[rip+0x2b81344]        # 0x4b62690
 1fe134c:	e8 ab 74 5d fe                                  	call   0x5b87fc
 1fe1351:	0f 28 45 30                                     	movaps xmm0,XMMWORD PTR [rbp+0x30]
 1fe1355:	48 8d 44 24 50                                  	lea    rax,[rsp+0x50]
 1fe135a:	41 b9 58 01 00 00                               	mov    r9d,0x158
 1fe1360:	66 0f 7f 44 24 50                               	movdqa XMMWORD PTR [rsp+0x50],xmm0
 1fe1366:	41 8b d1                                        	mov    edx,r9d
 1fe1369:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
 1fe136e:	4c 8b c3                                        	mov    r8,rbx
 1fe1371:	49 8b ce                                        	mov    rcx,r14
 1fe1374:	e8 9f 71 5d fe                                  	call   0x5b8518
 1fe1379:	85 c0                                           	test   eax,eax
 1fe137b:	75 0a                                           	jne    0x1fe1387
 1fe137d:	c7 86 b8 00 00 00 0d 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0xd
 1fe1387:	ba 02 00 00 00                                  	mov    edx,0x2
 1fe138c:	48 8b cb                                        	mov    rcx,rbx
 1fe138f:	49 8b c6                                        	mov    rax,r14
 1fe1392:	44 8d 42 7e                                     	lea    r8d,[rdx+0x7e]
 1fe1396:	0f 10 00                                        	movups xmm0,XMMWORD PTR [rax]
 1fe1399:	0f 11 01                                        	movups XMMWORD PTR [rcx],xmm0
 1fe139c:	0f 10 48 10                                     	movups xmm1,XMMWORD PTR [rax+0x10]
 1fe13a0:	0f 11 49 10                                     	movups XMMWORD PTR [rcx+0x10],xmm1
 1fe13a4:	0f 10 40 20                                     	movups xmm0,XMMWORD PTR [rax+0x20]
 1fe13a8:	0f 11 41 20                                     	movups XMMWORD PTR [rcx+0x20],xmm0
 1fe13ac:	0f 10 48 30                                     	movups xmm1,XMMWORD PTR [rax+0x30]
 1fe13b0:	0f 11 49 30                                     	movups XMMWORD PTR [rcx+0x30],xmm1
 1fe13b4:	0f 10 40 40                                     	movups xmm0,XMMWORD PTR [rax+0x40]
 1fe13b8:	0f 11 41 40                                     	movups XMMWORD PTR [rcx+0x40],xmm0
 1fe13bc:	0f 10 48 50                                     	movups xmm1,XMMWORD PTR [rax+0x50]
 1fe13c0:	0f 11 49 50                                     	movups XMMWORD PTR [rcx+0x50],xmm1
 1fe13c4:	0f 10 40 60                                     	movups xmm0,XMMWORD PTR [rax+0x60]
 1fe13c8:	0f 11 41 60                                     	movups XMMWORD PTR [rcx+0x60],xmm0
 1fe13cc:	49 03 c8                                        	add    rcx,r8
 1fe13cf:	0f 10 48 70                                     	movups xmm1,XMMWORD PTR [rax+0x70]
 1fe13d3:	49 03 c0                                        	add    rax,r8
 1fe13d6:	0f 11 49 f0                                     	movups XMMWORD PTR [rcx-0x10],xmm1
 1fe13da:	48 83 ea 01                                     	sub    rdx,0x1
 1fe13de:	75 b6                                           	jne    0x1fe1396
 1fe13e0:	0f 10 00                                        	movups xmm0,XMMWORD PTR [rax]
 1fe13e3:	48 8d 55 00                                     	lea    rdx,[rbp+0x0]
 1fe13e7:	0f 11 01                                        	movups XMMWORD PTR [rcx],xmm0
 1fe13ea:	0f 10 48 10                                     	movups xmm1,XMMWORD PTR [rax+0x10]
 1fe13ee:	0f 11 49 10                                     	movups XMMWORD PTR [rcx+0x10],xmm1
 1fe13f2:	0f 10 40 20                                     	movups xmm0,XMMWORD PTR [rax+0x20]
 1fe13f6:	0f 11 41 20                                     	movups XMMWORD PTR [rcx+0x20],xmm0
 1fe13fa:	0f 10 48 30                                     	movups xmm1,XMMWORD PTR [rax+0x30]
 1fe13fe:	0f 11 49 30                                     	movups XMMWORD PTR [rcx+0x30],xmm1
 1fe1402:	0f 10 40 40                                     	movups xmm0,XMMWORD PTR [rax+0x40]
 1fe1406:	0f 11 41 40                                     	movups XMMWORD PTR [rcx+0x40],xmm0
 1fe140a:	48 8b 40 50                                     	mov    rax,QWORD PTR [rax+0x50]
 1fe140e:	48 89 41 50                                     	mov    QWORD PTR [rcx+0x50],rax
 1fe1412:	48 8d 0d 77 12 b8 02                            	lea    rcx,[rip+0x2b81277]        # 0x4b62690
 1fe1419:	e8 de 73 5d fe                                  	call   0x5b87fc
 1fe141e:	0f 28 45 10                                     	movaps xmm0,XMMWORD PTR [rbp+0x10]
 1fe1422:	48 8d 44 24 50                                  	lea    rax,[rsp+0x50]
 1fe1427:	41 b9 58 01 00 00                               	mov    r9d,0x158
 1fe142d:	66 0f 7f 44 24 50                               	movdqa XMMWORD PTR [rsp+0x50],xmm0
 1fe1433:	41 8b d1                                        	mov    edx,r9d
 1fe1436:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
 1fe143b:	4c 8b c3                                        	mov    r8,rbx
 1fe143e:	49 8b ce                                        	mov    rcx,r14
 1fe1441:	e8 d2 70 5d fe                                  	call   0x5b8518
 1fe1446:	85 c0                                           	test   eax,eax
 1fe1448:	75 0a                                           	jne    0x1fe1454
 1fe144a:	c7 86 b8 00 00 00 0d 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0xd
 1fe1454:	ba 02 00 00 00                                  	mov    edx,0x2
 1fe1459:	48 8b cb                                        	mov    rcx,rbx
 1fe145c:	49 8b c6                                        	mov    rax,r14
 1fe145f:	44 8d 42 7e                                     	lea    r8d,[rdx+0x7e]
 1fe1463:	0f 10 00                                        	movups xmm0,XMMWORD PTR [rax]
 1fe1466:	0f 11 01                                        	movups XMMWORD PTR [rcx],xmm0
 1fe1469:	0f 10 48 10                                     	movups xmm1,XMMWORD PTR [rax+0x10]
 1fe146d:	0f 11 49 10                                     	movups XMMWORD PTR [rcx+0x10],xmm1
 1fe1471:	0f 10 40 20                                     	movups xmm0,XMMWORD PTR [rax+0x20]
 1fe1475:	0f 11 41 20                                     	movups XMMWORD PTR [rcx+0x20],xmm0
 1fe1479:	0f 10 48 30                                     	movups xmm1,XMMWORD PTR [rax+0x30]
 1fe147d:	0f 11 49 30                                     	movups XMMWORD PTR [rcx+0x30],xmm1
 1fe1481:	0f 10 40 40                                     	movups xmm0,XMMWORD PTR [rax+0x40]
 1fe1485:	0f 11 41 40                                     	movups XMMWORD PTR [rcx+0x40],xmm0
 1fe1489:	0f 10 48 50                                     	movups xmm1,XMMWORD PTR [rax+0x50]
 1fe148d:	0f 11 49 50                                     	movups XMMWORD PTR [rcx+0x50],xmm1
 1fe1491:	0f 10 40 60                                     	movups xmm0,XMMWORD PTR [rax+0x60]
 1fe1495:	0f 11 41 60                                     	movups XMMWORD PTR [rcx+0x60],xmm0
 1fe1499:	49 03 c8                                        	add    rcx,r8
 1fe149c:	0f 10 48 70                                     	movups xmm1,XMMWORD PTR [rax+0x70]
 1fe14a0:	49 03 c0                                        	add    rax,r8
 1fe14a3:	0f 11 49 f0                                     	movups XMMWORD PTR [rcx-0x10],xmm1
 1fe14a7:	48 83 ea 01                                     	sub    rdx,0x1
 1fe14ab:	75 b6                                           	jne    0x1fe1463
 1fe14ad:	0f 10 00                                        	movups xmm0,XMMWORD PTR [rax]
 1fe14b0:	49 8b d6                                        	mov    rdx,r14
 1fe14b3:	0f 11 01                                        	movups XMMWORD PTR [rcx],xmm0
 1fe14b6:	0f 10 48 10                                     	movups xmm1,XMMWORD PTR [rax+0x10]
 1fe14ba:	0f 11 49 10                                     	movups XMMWORD PTR [rcx+0x10],xmm1
 1fe14be:	0f 10 40 20                                     	movups xmm0,XMMWORD PTR [rax+0x20]
 1fe14c2:	0f 11 41 20                                     	movups XMMWORD PTR [rcx+0x20],xmm0
 1fe14c6:	0f 10 48 30                                     	movups xmm1,XMMWORD PTR [rax+0x30]
 1fe14ca:	0f 11 49 30                                     	movups XMMWORD PTR [rcx+0x30],xmm1
 1fe14ce:	0f 10 40 40                                     	movups xmm0,XMMWORD PTR [rax+0x40]
 1fe14d2:	0f 11 41 40                                     	movups XMMWORD PTR [rcx+0x40],xmm0
 1fe14d6:	48 8b 40 50                                     	mov    rax,QWORD PTR [rax+0x50]
 1fe14da:	48 89 41 50                                     	mov    QWORD PTR [rcx+0x50],rax
 1fe14de:	48 8b cf                                        	mov    rcx,rdi
 1fe14e1:	48 8b 07                                        	mov    rax,QWORD PTR [rdi]
 1fe14e4:	ff 50 58                                        	call   QWORD PTR [rax+0x58]
 1fe14e7:	48 8d 4d 00                                     	lea    rcx,[rbp+0x0]
 1fe14eb:	e8 c0 85 fd fe                                  	call   0xfb9ab0
 1fe14f0:	48 8d 4d 10                                     	lea    rcx,[rbp+0x10]
 1fe14f4:	e8 b7 85 fd fe                                  	call   0xfb9ab0
 1fe14f9:	48 8d 4d 20                                     	lea    rcx,[rbp+0x20]
 1fe14fd:	e8 ae 85 fd fe                                  	call   0xfb9ab0
 1fe1502:	48 8d 4d 30                                     	lea    rcx,[rbp+0x30]
 1fe1506:	e8 a5 85 fd fe                                  	call   0xfb9ab0
 1fe150b:	eb 45                                           	jmp    0x1fe1552
 1fe150d:	48 8d 4d 70                                     	lea    rcx,[rbp+0x70]
 1fe1511:	e8 ae 4e ff ff                                  	call   0x1fd63c4
 1fe1516:	4c 8b f8                                        	mov    r15,rax
 1fe1519:	48 85 c0                                        	test   rax,rax
 1fe151c:	75 0f                                           	jne    0x1fe152d
 1fe151e:	c7 86 b8 00 00 00 03 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0x3
 1fe1528:	e9 e1 04 00 00                                  	jmp    0x1fe1a0e
 1fe152d:	48 3d 00 00 b0 00                               	cmp    rax,0xb00000
 1fe1533:	0f 82 66 fd ff ff                               	jb     0x1fe129f
 1fe1539:	c7 86 b8 00 00 00 07 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0x7
 1fe1543:	e9 c6 04 00 00                                  	jmp    0x1fe1a0e
 1fe1548:	c7 86 b8 00 00 00 04 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0x4
 1fe1552:	44 8a 8e 27 0e 00 00                            	mov    r9b,BYTE PTR [rsi+0xe27]
 1fe1559:	48 8d 05 68 f5 90 01                            	lea    rax,[rip+0x190f568]        # 0x38f0ac8
 1fe1560:	45 84 c9                                        	test   r9b,r9b
 1fe1563:	48 8d 0d 16 dd c4 01                            	lea    rcx,[rip+0x1c4dd16]        # 0x3c2f280
 1fe156a:	48 0f 44 c8                                     	cmove  rcx,rax
 1fe156e:	48 8b c3                                        	mov    rax,rbx
 1fe1571:	48 2b cb                                        	sub    rcx,rbx
 1fe1574:	44 0f b6 00                                     	movzx  r8d,BYTE PTR [rax]
 1fe1578:	0f b6 14 08                                     	movzx  edx,BYTE PTR [rax+rcx*1]
 1fe157c:	44 2b c2                                        	sub    r8d,edx
 1fe157f:	75 07                                           	jne    0x1fe1588
 1fe1581:	48 ff c0                                        	inc    rax
 1fe1584:	85 d2                                           	test   edx,edx
 1fe1586:	75 ec                                           	jne    0x1fe1574
 1fe1588:	45 85 c0                                        	test   r8d,r8d
 1fe158b:	75 22                                           	jne    0x1fe15af
 1fe158d:	8b 43 1c                                        	mov    eax,DWORD PTR [rbx+0x1c]
 1fe1590:	48 39 86 e8 00 00 00                            	cmp    QWORD PTR [rsi+0xe8],rax
 1fe1597:	75 16                                           	jne    0x1fe15af
 1fe1599:	48 8b 43 10                                     	mov    rax,QWORD PTR [rbx+0x10]
 1fe159d:	49 39 04 24                                     	cmp    QWORD PTR [r12],rax
 1fe15a1:	74 16                                           	je     0x1fe15b9
 1fe15a3:	c7 86 b8 00 00 00 0b 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0xb
 1fe15ad:	eb 0a                                           	jmp    0x1fe15b9
 1fe15af:	c7 86 b8 00 00 00 07 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0x7
 1fe15b9:	8b 86 b8 00 00 00                               	mov    eax,DWORD PTR [rsi+0xb8]
 1fe15bf:	45 33 e4                                        	xor    r12d,r12d
 1fe15c2:	85 c0                                           	test   eax,eax
 1fe15c4:	0f 85 18 04 00 00                               	jne    0x1fe19e2
 1fe15ca:	41 8b c4                                        	mov    eax,r12d
 1fe15cd:	45 84 c9                                        	test   r9b,r9b
 1fe15d0:	0f 85 bb 00 00 00                               	jne    0x1fe1691
 1fe15d6:	ff 86 f0 e8 00 00                               	inc    DWORD PTR [rsi+0xe8f0]
 1fe15dc:	48 8d 4d 70                                     	lea    rcx,[rbp+0x70]
 1fe15e0:	8b 43 38                                        	mov    eax,DWORD PTR [rbx+0x38]
 1fe15e3:	44 39 63 44                                     	cmp    DWORD PTR [rbx+0x44],r12d
 1fe15e7:	89 44 24 68                                     	mov    DWORD PTR [rsp+0x68],eax
 1fe15eb:	8b 43 3c                                        	mov    eax,DWORD PTR [rbx+0x3c]
 1fe15ee:	0f 95 44 24 75                                  	setne  BYTE PTR [rsp+0x75]
 1fe15f3:	89 44 24 6c                                     	mov    DWORD PTR [rsp+0x6c],eax
 1fe15f7:	8b 83 8c 00 00 00                               	mov    eax,DWORD PTR [rbx+0x8c]
 1fe15fd:	89 44 24 70                                     	mov    DWORD PTR [rsp+0x70],eax
 1fe1601:	8a 43 40                                        	mov    al,BYTE PTR [rbx+0x40]
 1fe1604:	88 44 24 74                                     	mov    BYTE PTR [rsp+0x74],al
 1fe1608:	8a 43 48                                        	mov    al,BYTE PTR [rbx+0x48]
 1fe160b:	88 44 24 76                                     	mov    BYTE PTR [rsp+0x76],al
 1fe160f:	8a 83 90 00 00 00                               	mov    al,BYTE PTR [rbx+0x90]
 1fe1615:	88 44 24 77                                     	mov    BYTE PTR [rsp+0x77],al
 1fe1619:	8b 43 28                                        	mov    eax,DWORD PTR [rbx+0x28]
 1fe161c:	89 44 24 78                                     	mov    DWORD PTR [rsp+0x78],eax
 1fe1620:	48 8b 43 30                                     	mov    rax,QWORD PTR [rbx+0x30]
 1fe1624:	48 89 45 80                                     	mov    QWORD PTR [rbp-0x80],rax
 1fe1628:	48 8b 43 20                                     	mov    rax,QWORD PTR [rbx+0x20]
 1fe162c:	48 89 45 88                                     	mov    QWORD PTR [rbp-0x78],rax
 1fe1630:	e8 8f 4d ff ff                                  	call   0x1fd63c4
 1fe1635:	0f 10 44 24 68                                  	movups xmm0,XMMWORD PTR [rsp+0x68]
 1fe163a:	48 89 45 90                                     	mov    QWORD PTR [rbp-0x70],rax
 1fe163e:	48 8d 86 18 01 00 00                            	lea    rax,[rsi+0x118]
 1fe1645:	0f 10 4c 24 78                                  	movups xmm1,XMMWORD PTR [rsp+0x78]
 1fe164a:	49 c1 e5 06                                     	shl    r13,0x6
 1fe164e:	49 03 c5                                        	add    rax,r13
 1fe1651:	48 89 45 98                                     	mov    QWORD PTR [rbp-0x68],rax
 1fe1655:	8b 86 f0 e8 00 00                               	mov    eax,DWORD PTR [rsi+0xe8f0]
 1fe165b:	ff c8                                           	dec    eax
 1fe165d:	48 6b c8 38                                     	imul   rcx,rax,0x38
 1fe1661:	0f 11 84 31 30 0e 00 00                         	movups XMMWORD PTR [rcx+rsi*1+0xe30],xmm0
 1fe1669:	0f 10 45 88                                     	movups xmm0,XMMWORD PTR [rbp-0x78]
 1fe166d:	0f 11 8c 31 40 0e 00 00                         	movups XMMWORD PTR [rcx+rsi*1+0xe40],xmm1
 1fe1675:	f2 0f 10 4d 98                                  	movsd  xmm1,QWORD PTR [rbp-0x68]
 1fe167a:	0f 11 84 31 50 0e 00 00                         	movups XMMWORD PTR [rcx+rsi*1+0xe50],xmm0
 1fe1682:	f2 0f 11 8c 31 60 0e 00 00                      	movsd  QWORD PTR [rcx+rsi*1+0xe60],xmm1
 1fe168b:	8b 86 b8 00 00 00                               	mov    eax,DWORD PTR [rsi+0xb8]
 1fe1691:	44 8a 74 24 40                                  	mov    r14b,BYTE PTR [rsp+0x40]
 1fe1696:	45 84 f6                                        	test   r14b,r14b
 1fe1699:	0f 85 0e 03 00 00                               	jne    0x1fe19ad
 1fe169f:	85 c0                                           	test   eax,eax
 1fe16a1:	0f 85 06 03 00 00                               	jne    0x1fe19ad
 1fe16a7:	48 8d 83 58 01 00 00                            	lea    rax,[rbx+0x158]
 1fe16ae:	48 85 c0                                        	test   rax,rax
 1fe16b1:	0f 84 f6 02 00 00                               	je     0x1fe19ad
 1fe16b7:	e8 dc f3 14 fe                                  	call   0x130a98
 1fe16bc:	48 8b 96 e8 00 00 00                            	mov    rdx,QWORD PTR [rsi+0xe8]
 1fe16c3:	4c 8d 45 c0                                     	lea    r8,[rbp-0x40]
 1fe16c7:	45 33 ed                                        	xor    r13d,r13d
 1fe16ca:	c7 45 c0 35 00 00 00                            	mov    DWORD PTR [rbp-0x40],0x35
 1fe16d1:	4c 8b e0                                        	mov    r12,rax
 1fe16d4:	4c 89 6d c8                                     	mov    QWORD PTR [rbp-0x38],r13
 1fe16d8:	48 8b 08                                        	mov    rcx,QWORD PTR [rax]
 1fe16db:	4c 8b 49 28                                     	mov    r9,QWORD PTR [rcx+0x28]
 1fe16df:	48 8b c8                                        	mov    rcx,rax
 1fe16e2:	41 ff d1                                        	call   r9
 1fe16e5:	4c 8b f0                                        	mov    r14,rax
 1fe16e8:	48 85 c0                                        	test   rax,rax
 1fe16eb:	0f 84 e5 02 00 00                               	je     0x1fe19d6
 1fe16f1:	45 8b d5                                        	mov    r10d,r13d
 1fe16f4:	4c 8d 1d 05 e9 01 fe                            	lea    r11,[rip+0xfffffffffe01e905]        # 0x0
 1fe16fb:	47 8a 84 1a d0 4b bd 03                         	mov    r8b,BYTE PTR [r10+r11*1+0x3bd4bd0]
 1fe1703:	47 8a 8c 1a c8 4b bd 03                         	mov    r9b,BYTE PTR [r10+r11*1+0x3bd4bc8]
 1fe170b:	41 8a c0                                        	mov    al,r8b
 1fe170e:	43 32 84 1a b4 78 bd 03                         	xor    al,BYTE PTR [r10+r11*1+0x3bd78b4]
 1fe1716:	41 8a c9                                        	mov    cl,r9b
 1fe1719:	43 32 8c 1a b0 78 bd 03                         	xor    cl,BYTE PTR [r10+r11*1+0x3bd78b0]
 1fe1721:	43 8a 94 1a cc 4b bd 03                         	mov    dl,BYTE PTR [r10+r11*1+0x3bd4bcc]
 1fe1729:	47 32 8c 1a c0 78 bd 03                         	xor    r9b,BYTE PTR [r10+r11*1+0x3bd78c0]
 1fe1731:	47 32 84 1a c4 78 bd 03                         	xor    r8b,BYTE PTR [r10+r11*1+0x3bd78c4]
 1fe1739:	42 88 44 15 d4                                  	mov    BYTE PTR [rbp+r10*1-0x2c],al
 1fe173e:	8a c2                                           	mov    al,dl
 1fe1740:	43 32 84 1a b8 78 bd 03                         	xor    al,BYTE PTR [r10+r11*1+0x3bd78b8]
 1fe1748:	43 32 94 1a c8 78 bd 03                         	xor    dl,BYTE PTR [r10+r11*1+0x3bd78c8]
 1fe1750:	42 88 4c 15 d0                                  	mov    BYTE PTR [rbp+r10*1-0x30],cl
 1fe1755:	43 8a 8c 1a d4 4b bd 03                         	mov    cl,BYTE PTR [r10+r11*1+0x3bd4bd4]
 1fe175d:	42 88 44 15 d8                                  	mov    BYTE PTR [rbp+r10*1-0x28],al
 1fe1762:	8a c1                                           	mov    al,cl
 1fe1764:	43 32 84 1a bc 78 bd 03                         	xor    al,BYTE PTR [r10+r11*1+0x3bd78bc]
 1fe176c:	43 32 8c 1a cc 78 bd 03                         	xor    cl,BYTE PTR [r10+r11*1+0x3bd78cc]
 1fe1774:	42 88 44 15 dc                                  	mov    BYTE PTR [rbp+r10*1-0x24],al
 1fe1779:	46 88 4c 15 e0                                  	mov    BYTE PTR [rbp+r10*1-0x20],r9b
 1fe177e:	46 88 44 15 e4                                  	mov    BYTE PTR [rbp+r10*1-0x1c],r8b
 1fe1783:	42 88 54 15 e8                                  	mov    BYTE PTR [rbp+r10*1-0x18],dl
 1fe1788:	42 88 4c 15 ec                                  	mov    BYTE PTR [rbp+r10*1-0x14],cl
 1fe178d:	49 ff c2                                        	inc    r10
 1fe1790:	49 83 fa 04                                     	cmp    r10,0x4
 1fe1794:	0f 8c 61 ff ff ff                               	jl     0x1fe16fb
 1fe179a:	48 8d 55 d0                                     	lea    rdx,[rbp-0x30]
 1fe179e:	48 8d 0d eb 0e b8 02                            	lea    rcx,[rip+0x2b80eeb]        # 0x4b62690
 1fe17a5:	e8 52 70 5d fe                                  	call   0x5b87fc
 1fe17aa:	0f 28 75 e0                                     	movaps xmm6,XMMWORD PTR [rbp-0x20]
 1fe17ae:	48 8d 4c 24 50                                  	lea    rcx,[rsp+0x50]
 1fe17b3:	ba 10 00 00 00                                  	mov    edx,0x10
 1fe17b8:	48 89 4c 24 20                                  	mov    QWORD PTR [rsp+0x20],rcx
 1fe17bd:	48 8d 4d f0                                     	lea    rcx,[rbp-0x10]
 1fe17c1:	66 0f 7f 74 24 50                               	movdqa XMMWORD PTR [rsp+0x50],xmm6
 1fe17c7:	4c 8d 43 49                                     	lea    r8,[rbx+0x49]
 1fe17cb:	44 8b ca                                        	mov    r9d,edx
 1fe17ce:	e8 45 6d 5d fe                                  	call   0x5b8518
 1fe17d3:	85 c0                                           	test   eax,eax
 1fe17d5:	75 0a                                           	jne    0x1fe17e1
 1fe17d7:	c7 86 b8 00 00 00 0d 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0xd
 1fe17e1:	48 8d 4c 24 50                                  	lea    rcx,[rsp+0x50]
 1fe17e6:	66 0f 7f 74 24 50                               	movdqa XMMWORD PTR [rsp+0x50],xmm6
 1fe17ec:	48 89 4c 24 20                                  	mov    QWORD PTR [rsp+0x20],rcx
 1fe17f1:	4c 8d 43 59                                     	lea    r8,[rbx+0x59]
 1fe17f5:	b9 10 00 00 00                                  	mov    ecx,0x10
 1fe17fa:	44 8b c9                                        	mov    r9d,ecx
 1fe17fd:	8b d1                                           	mov    edx,ecx
 1fe17ff:	48 8d 4d 40                                     	lea    rcx,[rbp+0x40]
 1fe1803:	e8 10 6d 5d fe                                  	call   0x5b8518
 1fe1808:	85 c0                                           	test   eax,eax
 1fe180a:	75 0a                                           	jne    0x1fe1816
 1fe180c:	c7 86 b8 00 00 00 0d 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0xd
 1fe1816:	48 8d 55 f0                                     	lea    rdx,[rbp-0x10]
 1fe181a:	48 8d 0d 6f 0e b8 02                            	lea    rcx,[rip+0x2b80e6f]        # 0x4b62690
 1fe1821:	e8 d6 6f 5d fe                                  	call   0x5b87fc
 1fe1826:	0f 28 75 40                                     	movaps xmm6,XMMWORD PTR [rbp+0x40]
 1fe182a:	48 8d 4c 24 50                                  	lea    rcx,[rsp+0x50]
 1fe182f:	48 89 4c 24 20                                  	mov    QWORD PTR [rsp+0x20],rcx
 1fe1834:	4c 8d 43 69                                     	lea    r8,[rbx+0x69]
 1fe1838:	b9 10 00 00 00                                  	mov    ecx,0x10
 1fe183d:	66 0f 7f 74 24 50                               	movdqa XMMWORD PTR [rsp+0x50],xmm6
 1fe1843:	44 8b c9                                        	mov    r9d,ecx
 1fe1846:	8b d1                                           	mov    edx,ecx
 1fe1848:	48 8d 4d 50                                     	lea    rcx,[rbp+0x50]
 1fe184c:	e8 c7 6c 5d fe                                  	call   0x5b8518
 1fe1851:	85 c0                                           	test   eax,eax
 1fe1853:	75 0a                                           	jne    0x1fe185f
 1fe1855:	c7 86 b8 00 00 00 0d 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0xd
 1fe185f:	48 8d 44 24 50                                  	lea    rax,[rsp+0x50]
 1fe1864:	66 0f 7f 74 24 50                               	movdqa XMMWORD PTR [rsp+0x50],xmm6
 1fe186a:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
 1fe186f:	4c 8d 43 79                                     	lea    r8,[rbx+0x79]
 1fe1873:	b8 10 00 00 00                                  	mov    eax,0x10
 1fe1878:	48 8d 4d 60                                     	lea    rcx,[rbp+0x60]
 1fe187c:	44 8b c8                                        	mov    r9d,eax
 1fe187f:	8b d0                                           	mov    edx,eax
 1fe1881:	e8 92 6c 5d fe                                  	call   0x5b8518
 1fe1886:	85 c0                                           	test   eax,eax
 1fe1888:	75 0a                                           	jne    0x1fe1894
 1fe188a:	c7 86 b8 00 00 00 0d 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0xd
 1fe1894:	48 8d 55 50                                     	lea    rdx,[rbp+0x50]
 1fe1898:	48 8d 0d f1 0d b8 02                            	lea    rcx,[rip+0x2b80df1]        # 0x4b62690
 1fe189f:	e8 58 6f 5d fe                                  	call   0x5b87fc
 1fe18a4:	0f 28 45 60                                     	movaps xmm0,XMMWORD PTR [rbp+0x60]
 1fe18a8:	48 8d 44 24 50                                  	lea    rax,[rsp+0x50]
 1fe18ad:	8b 96 e8 00 00 00                               	mov    edx,DWORD PTR [rsi+0xe8]
 1fe18b3:	4c 8d 83 58 01 00 00                            	lea    r8,[rbx+0x158]
 1fe18ba:	44 8b ca                                        	mov    r9d,edx
 1fe18bd:	66 0f 7f 44 24 50                               	movdqa XMMWORD PTR [rsp+0x50],xmm0
 1fe18c3:	49 8b ce                                        	mov    rcx,r14
 1fe18c6:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
 1fe18cb:	e8 48 6c 5d fe                                  	call   0x5b8518
 1fe18d0:	85 c0                                           	test   eax,eax
 1fe18d2:	75 0a                                           	jne    0x1fe18de
 1fe18d4:	c7 86 b8 00 00 00 0d 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0xd
 1fe18de:	4c 8b 86 e8 00 00 00                            	mov    r8,QWORD PTR [rsi+0xe8]
 1fe18e5:	49 8b d6                                        	mov    rdx,r14
 1fe18e8:	48 8b 8e e0 00 00 00                            	mov    rcx,QWORD PTR [rsi+0xe0]
 1fe18ef:	e8 2c 28 bb fe                                  	call   0xb94120
 1fe18f4:	48 8d 55 f0                                     	lea    rdx,[rbp-0x10]
 1fe18f8:	48 8d 0d 91 0d b8 02                            	lea    rcx,[rip+0x2b80d91]        # 0x4b62690
 1fe18ff:	e8 f8 6e 5d fe                                  	call   0x5b87fc
 1fe1904:	8b 96 e8 00 00 00                               	mov    edx,DWORD PTR [rsi+0xe8]
 1fe190a:	48 8d 44 24 50                                  	lea    rax,[rsp+0x50]
 1fe190f:	4c 8b 86 e0 00 00 00                            	mov    r8,QWORD PTR [rsi+0xe0]
 1fe1916:	44 8b ca                                        	mov    r9d,edx
 1fe1919:	49 8b ce                                        	mov    rcx,r14
 1fe191c:	48 89 44 24 20                                  	mov    QWORD PTR [rsp+0x20],rax
 1fe1921:	66 0f 7f 74 24 50                               	movdqa XMMWORD PTR [rsp+0x50],xmm6
 1fe1927:	e8 ec 6b 5d fe                                  	call   0x5b8518
 1fe192c:	85 c0                                           	test   eax,eax
 1fe192e:	75 0a                                           	jne    0x1fe193a
 1fe1930:	c7 86 b8 00 00 00 0d 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0xd
 1fe193a:	4c 8b 86 e8 00 00 00                            	mov    r8,QWORD PTR [rsi+0xe8]
 1fe1941:	49 8b d6                                        	mov    rdx,r14
 1fe1944:	48 8b 8e e0 00 00 00                            	mov    rcx,QWORD PTR [rsi+0xe0]
 1fe194b:	e8 d0 27 bb fe                                  	call   0xb94120
 1fe1950:	48 8d 4d f0                                     	lea    rcx,[rbp-0x10]
 1fe1954:	e8 57 81 fd fe                                  	call   0xfb9ab0
 1fe1959:	48 8d 4d 40                                     	lea    rcx,[rbp+0x40]
 1fe195d:	e8 4e 81 fd fe                                  	call   0xfb9ab0
 1fe1962:	48 8d 4d 50                                     	lea    rcx,[rbp+0x50]
 1fe1966:	e8 45 81 fd fe                                  	call   0xfb9ab0
 1fe196b:	48 8d 4d 60                                     	lea    rcx,[rbp+0x60]
 1fe196f:	e8 3c 81 fd fe                                  	call   0xfb9ab0
 1fe1974:	48 8d 4b 49                                     	lea    rcx,[rbx+0x49]
 1fe1978:	e8 33 81 fd fe                                  	call   0xfb9ab0
 1fe197d:	48 8d 4b 59                                     	lea    rcx,[rbx+0x59]
 1fe1981:	e8 2a 81 fd fe                                  	call   0xfb9ab0
 1fe1986:	48 8d 4b 69                                     	lea    rcx,[rbx+0x69]
 1fe198a:	e8 21 81 fd fe                                  	call   0xfb9ab0
 1fe198f:	48 8d 4b 79                                     	lea    rcx,[rbx+0x79]
 1fe1993:	e8 18 81 fd fe                                  	call   0xfb9ab0
 1fe1998:	49 8b 04 24                                     	mov    rax,QWORD PTR [r12]
 1fe199c:	49 8b d6                                        	mov    rdx,r14
 1fe199f:	49 8b cc                                        	mov    rcx,r12
 1fe19a2:	ff 50 58                                        	call   QWORD PTR [rax+0x58]
 1fe19a5:	45 33 e4                                        	xor    r12d,r12d
 1fe19a8:	44 8a 74 24 40                                  	mov    r14b,BYTE PTR [rsp+0x40]
 1fe19ad:	44 38 a6 27 0e 00 00                            	cmp    BYTE PTR [rsi+0xe27],r12b
 1fe19b4:	75 05                                           	jne    0x1fe19bb
 1fe19b6:	45 84 f6                                        	test   r14b,r14b
 1fe19b9:	74 6d                                           	je     0x1fe1a28
 1fe19bb:	4d 8b c7                                        	mov    r8,r15
 1fe19be:	33 d2                                           	xor    edx,edx
 1fe19c0:	48 8b cb                                        	mov    rcx,rbx
 1fe19c3:	e8 08 24 bb fe                                  	call   0xb93dd0
 1fe19c8:	48 8b 07                                        	mov    rax,QWORD PTR [rdi]
 1fe19cb:	48 8b d3                                        	mov    rdx,rbx
 1fe19ce:	48 8b cf                                        	mov    rcx,rdi
 1fe19d1:	ff 50 58                                        	call   QWORD PTR [rax+0x58]
 1fe19d4:	eb 3b                                           	jmp    0x1fe1a11
 1fe19d6:	c7 86 b8 00 00 00 04 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0x4
 1fe19e0:	eb c3                                           	jmp    0x1fe19a5
 1fe19e2:	45 84 c9                                        	test   r9b,r9b
 1fe19e5:	75 c1                                           	jne    0x1fe19a8
 1fe19e7:	83 f8 07                                        	cmp    eax,0x7
 1fe19ea:	75 bc                                           	jne    0x1fe19a8
 1fe19ec:	c6 86 29 0e 00 00 01                            	mov    BYTE PTR [rsi+0xe29],0x1
 1fe19f3:	eb b3                                           	jmp    0x1fe19a8
 1fe19f5:	c7 86 b8 00 00 00 03 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0x3
 1fe19ff:	45 33 e4                                        	xor    r12d,r12d
 1fe1a02:	eb a9                                           	jmp    0x1fe19ad
 1fe1a04:	c7 86 b8 00 00 00 04 00 00 00                   	mov    DWORD PTR [rsi+0xb8],0x4
 1fe1a0e:	45 33 e4                                        	xor    r12d,r12d
 1fe1a11:	8b 5c 24 60                                     	mov    ebx,DWORD PTR [rsp+0x60]
 1fe1a15:	ff c3                                           	inc    ebx
 1fe1a17:	89 5c 24 60                                     	mov    DWORD PTR [rsp+0x60],ebx
 1fe1a1b:	3b 9e d8 0d 00 00                               	cmp    ebx,DWORD PTR [rsi+0xdd8]
 1fe1a21:	73 1e                                           	jae    0x1fe1a41
 1fe1a23:	e9 7a f7 ff ff                                  	jmp    0x1fe11a2
 1fe1a28:	4d 8b c7                                        	mov    r8,r15
 1fe1a2b:	33 d2                                           	xor    edx,edx
 1fe1a2d:	48 8b cb                                        	mov    rcx,rbx
 1fe1a30:	e8 9b 23 bb fe                                  	call   0xb93dd0
 1fe1a35:	48 8b 07                                        	mov    rax,QWORD PTR [rdi]
 1fe1a38:	48 8b d3                                        	mov    rdx,rbx
 1fe1a3b:	48 8b cf                                        	mov    rcx,rdi
 1fe1a3e:	ff 50 58                                        	call   QWORD PTR [rax+0x58]
 1fe1a41:	44 39 a6 b8 00 00 00                            	cmp    DWORD PTR [rsi+0xb8],r12d
 1fe1a48:	0f 94 c0                                        	sete   al
 1fe1a4b:	48 8b 8d 80 02 00 00                            	mov    rcx,QWORD PTR [rbp+0x280]
 1fe1a52:	48 33 cc                                        	xor    rcx,rsp
 1fe1a55:	e8 56 f7 ba fe                                  	call   0xb911b0
 1fe1a5a:	4c 8d 9c 24 a0 03 00 00                         	lea    r11,[rsp+0x3a0]
 1fe1a62:	49 8b 5b 38                                     	mov    rbx,QWORD PTR [r11+0x38]
 1fe1a66:	49 8b 73 40                                     	mov    rsi,QWORD PTR [r11+0x40]
 1fe1a6a:	49 8b 7b 48                                     	mov    rdi,QWORD PTR [r11+0x48]
 1fe1a6e:	41 0f 28 73 f0                                  	movaps xmm6,XMMWORD PTR [r11-0x10]
 1fe1a73:	49 8b e3                                        	mov    rsp,r11
 1fe1a76:	41 5f                                           	pop    r15
 1fe1a78:	41 5e                                           	pop    r14
 1fe1a7a:	41 5d                                           	pop    r13
 1fe1a7c:	41 5c                                           	pop    r12
 1fe1a7e:	5d                                              	pop    rbp
 1fe1a7f:	c3                                              	ret
