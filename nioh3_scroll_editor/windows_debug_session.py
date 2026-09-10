"""Internal Windows x64 debug-event ownership for the live-add executor.

Structures follow the installed Windows SDK 10.0.26100.0 headers. No renderer
API exposes this module. Every context mutation occurs at a stopped debug event.
"""
import ctypes as C
from ctypes import wintypes as W

U64 = C.c_uint64
DWORD = C.c_uint32
WORD = C.c_uint16
HANDLE = C.c_void_p


class Context(C.Structure):
    _fields_ = ([(f'P{i}Home', U64) for i in range(1, 7)] +
                [('ContextFlags', DWORD), ('MxCsr', DWORD)] +
                [(name, WORD) for name in ('SegCs', 'SegDs', 'SegEs', 'SegFs', 'SegGs', 'SegSs')] +
                [('EFlags', DWORD)] +
                [(name, U64) for name in ('Dr0', 'Dr1', 'Dr2', 'Dr3', 'Dr6', 'Dr7',
                  'Rax', 'Rcx', 'Rdx', 'Rbx', 'Rsp', 'Rbp', 'Rsi', 'Rdi', 'R8', 'R9',
                  'R10', 'R11', 'R12', 'R13', 'R14', 'R15', 'Rip')] +
                [('FltSave', C.c_ubyte * 512), ('VectorState', C.c_ubyte * 416)] +
                [(name, U64) for name in ('VectorControl', 'DebugControl', 'LastBranchToRip',
                                         'LastBranchFromRip', 'LastExceptionToRip', 'LastExceptionFromRip')])


class ExceptionRecord(C.Structure):
    _fields_ = [('code', DWORD), ('flags', DWORD), ('record', HANDLE),
                ('address', HANDLE), ('count', DWORD), ('information', U64 * 15)]


class ExceptionInfo(C.Structure):
    _fields_ = [('record', ExceptionRecord), ('first_chance', DWORD)]


class CreateThread(C.Structure):
    _fields_ = [('thread', HANDLE), ('tls', HANDLE), ('start', HANDLE)]


class CreateProcess(C.Structure):
    _fields_ = [('file', HANDLE), ('process', HANDLE), ('thread', HANDLE),
                ('base', HANDLE), ('debug_offset', DWORD), ('debug_size', DWORD),
                ('tls', HANDLE), ('start', HANDLE), ('name', HANDLE), ('unicode', WORD)]


class EventData(C.Union):
    _fields_ = [('exception', ExceptionInfo), ('thread', CreateThread),
                ('process', CreateProcess), ('file', HANDLE), ('exit_code', DWORD)]


class DebugEvent(C.Structure):
    _fields_ = [('code', DWORD), ('pid', DWORD), ('tid', DWORD), ('data', EventData)]


assert C.sizeof(Context) == 1232 and Context.Rip.offset == 248 and Context.Dr0.offset == 72
assert C.sizeof(DebugEvent) == 176 and DebugEvent.data.offset == 16


class WindowsDebug:
    def __init__(self, pid):
        self.pid = pid
        self.dll = C.WinDLL('kernel32', use_last_error=True)
        self.attached = False
        self.threads = {}
        self.original_debug = {}
        self.context_buffers = {}
        definitions = {
            'OpenProcess': ([DWORD, W.BOOL, DWORD], HANDLE),
            'CloseHandle': ([HANDLE], W.BOOL),
            'ReadProcessMemory': ([HANDLE, HANDLE, HANDLE, C.c_size_t, C.POINTER(C.c_size_t)], W.BOOL),
            'WriteProcessMemory': ([HANDLE, HANDLE, HANDLE, C.c_size_t, C.POINTER(C.c_size_t)], W.BOOL),
            'VirtualAllocEx': ([HANDLE, HANDLE, C.c_size_t, DWORD, DWORD], HANDLE),
            'VirtualFreeEx': ([HANDLE, HANDLE, C.c_size_t, DWORD], W.BOOL),
            'FlushInstructionCache': ([HANDLE, HANDLE, C.c_size_t], W.BOOL),
            'GetThreadContext': ([HANDLE, C.POINTER(Context)], W.BOOL),
            'SetThreadContext': ([HANDLE, C.POINTER(Context)], W.BOOL),
            'DebugActiveProcess': ([DWORD], W.BOOL),
            'DebugActiveProcessStop': ([DWORD], W.BOOL),
            'DebugSetProcessKillOnExit': ([W.BOOL], W.BOOL),
            'WaitForDebugEvent': ([C.POINTER(DebugEvent), DWORD], W.BOOL),
            'ContinueDebugEvent': ([DWORD, DWORD, DWORD], W.BOOL),
            'WaitForSingleObject': ([HANDLE, DWORD], DWORD),
            'DebugBreakProcess': ([HANDLE], W.BOOL),
        }
        for name, (arguments, result) in definitions.items():
            function = getattr(self.dll, name)
            function.argtypes, function.restype = arguments, result
        self.process = self.dll.OpenProcess(0x043A, False, pid)
        self.require(self.process, 'OpenProcess')

    @staticmethod
    def require(ok, operation):
        if not ok:
            raise C.WinError(C.get_last_error(), operation)

    def read(self, address, size):
        buffer, count = C.create_string_buffer(size), C.c_size_t()
        self.require(self.dll.ReadProcessMemory(self.process, address, buffer, size, C.byref(count)), 'ReadProcessMemory')
        if count.value != size:
            raise OSError('Short process read')
        return buffer.raw

    def write(self, address, value):
        value = bytes(value)
        buffer, count = C.create_string_buffer(value, len(value)), C.c_size_t()
        self.require(self.dll.WriteProcessMemory(self.process, address, buffer, len(value), C.byref(count)), 'WriteProcessMemory')
        if count.value != len(value):
            raise OSError('Short process write')

    def allocate(self, size=4096):
        address = self.dll.VirtualAllocEx(self.process, None, size, 0x3000, 0x40)
        self.require(address, 'VirtualAllocEx')
        return address

    def free(self, address):
        self.require(self.dll.VirtualFreeEx(self.process, address, 0, 0x8000), 'VirtualFreeEx')

    def attach(self):
        self.require(self.dll.DebugActiveProcess(self.pid), 'DebugActiveProcess')
        self.attached = True
        self.require(self.dll.DebugSetProcessKillOnExit(False), 'DebugSetProcessKillOnExit')

    def wait(self, milliseconds=100):
        event = DebugEvent()
        if self.dll.WaitForDebugEvent(C.byref(event), milliseconds):
            return event
        if C.get_last_error() in (121, 258):
            return None
        raise C.WinError(C.get_last_error(), 'WaitForDebugEvent')

    def resume(self, event, handled=True):
        self.require(self.dll.ContinueDebugEvent(event.pid, event.tid, 0x10002 if handled else 0x80010001), 'ContinueDebugEvent')

    def context(self, tid):
        # Windows requires 16-byte alignment; ctypes Structure alignment is 8.
        storage = C.create_string_buffer(C.sizeof(Context) + 15)
        context = Context.from_address((C.addressof(storage) + 15) & ~15)
        context.ContextFlags = 0x10001F
        self.context_buffers[tid] = storage
        self.require(self.dll.GetThreadContext(self.threads[tid], C.byref(context)), 'GetThreadContext')
        return context

    def set_context(self, tid, context):
        self.require(self.dll.SetThreadContext(self.threads[tid], C.byref(context)), 'SetThreadContext')

    def arm_thread(self, tid, handle, entry, acknowledgement):
        self.threads[tid] = handle
        context = self.context(tid)
        if context.Dr7 & 0xFF:
            raise RuntimeError('A thread already has active hardware breakpoints')
        self.original_debug[tid] = tuple(getattr(context, name) for name in ('Dr0', 'Dr1', 'Dr2', 'Dr3', 'Dr6', 'Dr7'))
        context.Dr0, context.Dr1 = entry, acknowledgement
        context.Dr6 = 0
        context.Dr7 = (context.Dr7 & ~0xFFFF00FF) | 5
        self.set_context(tid, context)

    def restore_threads(self):
        # Caller must own a stopped event, which suspends all debuggee threads.
        for tid, values in list(self.original_debug.items()):
            if tid not in self.threads:
                continue
            if self.dll.WaitForSingleObject(self.threads[tid], 0) == 0:
                continue  # A terminated thread has no live debug-register state.
            context = self.context(tid)
            for name, value in zip(('Dr0', 'Dr1', 'Dr2', 'Dr3', 'Dr6', 'Dr7'), values):
                setattr(context, name, value)
            self.set_context(tid, context)
        self.original_debug.clear()

    def all_threads_exited(self):
        return bool(self.threads) and all(self.dll.WaitForSingleObject(handle, 0) == 0
                                          for handle in self.threads.values())

    def close(self):
        if self.original_debug:
            raise RuntimeError('Debug-register restoration is not confirmed')
        if self.attached:
            self.require(self.dll.DebugActiveProcessStop(self.pid), 'DebugActiveProcessStop')
            self.attached = False
        for handle in self.threads.values():
            self.dll.CloseHandle(handle)
        self.threads.clear()
        if self.process:
            self.dll.CloseHandle(self.process)
            self.process = None
