import ctypes
import sys
import unittest
from nioh3_scroll_editor.runtime_challenge_override import ChallengeOverrideProfile, build_challenge_trampoline, CAPACITY_SIGNATURE, OverrideGroup

class ChallengeOverrideTests(unittest.TestCase):
    @unittest.skipUnless(sys.platform == 'win32', 'Windows x64 calling convention')
    def test_emitted_code_returns_only_for_target_seed_and_preserves_other_seed_path(self):
        dll=ctypes.WinDLL('kernel32',use_last_error=True)
        dll.VirtualAlloc.restype=ctypes.c_void_p
        dll.VirtualAlloc.argtypes=[ctypes.c_void_p,ctypes.c_size_t,ctypes.c_uint32,ctypes.c_uint32]
        dll.VirtualFree.argtypes=[ctypes.c_void_p,ctypes.c_size_t,ctypes.c_uint32]
        base=dll.VirtualAlloc(None,4096,0x3000,0x40)
        self.assertTrue(base)
        try:
            counter=base+0x300
            # Stand-in original function returns 4 after its relocated entry.
            ctypes.memmove(base+0x200,b'\xb8\x04\x00\x00\x00\xc3',6)
            code=build_challenge_trampoline(ChallengeOverrideProfile(10030565,7),return_address=base+0x200,counter_address=counter,original_instruction=CAPACITY_SIGNATURE)
            ctypes.memmove(base,code,len(code))
            call=ctypes.WINFUNCTYPE(ctypes.c_uint32,ctypes.c_uint64,ctypes.c_uint32)(base)
            self.assertEqual(call(0,10030565),7)
            self.assertEqual(call(0,10030566),4)
            self.assertEqual(call(0,10030565),7)
            self.assertEqual(ctypes.c_uint64.from_address(counter).value,2)
        finally:
            dll.VirtualFree(base,0,0x8000)

    def test_range_is_not_confused_with_unsigned_remaining_byte(self):
        for value in (-1,0,8,255):
            with self.assertRaises(ValueError):ChallengeOverrideProfile(100,value)

    def test_composite_cleanup_attempts_every_owner_and_retains_failures(self):
        calls=[]
        class Session:
            def __init__(self,fail):self.fail=fail
            def stop(self):
                calls.append(self.fail)
                if self.fail:raise OSError('Restore failed')
        group=OverrideGroup([Session(False),Session(True)])
        with self.assertRaises(OSError):group.stop()
        self.assertEqual(calls,[True,False])
