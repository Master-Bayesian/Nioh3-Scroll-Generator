"""Bounded typed named-pipe client for the optional local CE executor.

No Lua source or arbitrary command is accepted. Connection configuration stays
in the protected host environment, never in renderer parameters.
"""
import json
import os
import re
import struct
import time


MAX_FRAME = 512 * 1024


class NonblockingPipe:
    """Bounded local I/O even if CE's request thread stops responding."""
    def __init__(self, path, timeout=5):
        import ctypes
        import msvcrt
        from ctypes import wintypes
        self.deadline = time.monotonic() + timeout
        while True:
            try:
                self.stream = open(path, 'r+b', buffering=0)
                break
            except OSError as error:
                if getattr(error, 'winerror', None) not in (2, 231) or time.monotonic() >= self.deadline:
                    raise
                time.sleep(0.02)
        dll = ctypes.WinDLL('kernel32', use_last_error=True)
        dll.SetNamedPipeHandleState.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p, ctypes.c_void_p]
        dll.SetNamedPipeHandleState.restype = wintypes.BOOL
        mode = wintypes.DWORD(1)  # PIPE_NOWAIT; byte read mode.
        if not dll.SetNamedPipeHandleState(msvcrt.get_osfhandle(self.stream.fileno()), ctypes.byref(mode), None, None):
            self.stream.close()
            raise ctypes.WinError(ctypes.get_last_error())

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.stream.close()

    def transfer(self, method, value):
        while time.monotonic() < self.deadline:
            try:
                result = getattr(self.stream, method)(value)
                if result:
                    return result
            except OSError as error:
                if getattr(error, 'winerror', None) not in (232, 536):
                    raise
            time.sleep(0.01)
        raise TimeoutError('Executor I/O timed out; query the operation before retrying')

    def read(self, size):
        return self.transfer('read', size)

    def write(self, value):
        return self.transfer('write', value)


def encode_request(method, token, fields=None):
    if method not in ('ping', 'preview', 'insert', 'status', 'release', 'stop'):
        raise ValueError('Unknown live-add executor operation')
    if not re.fullmatch(r'[0-9a-f]{64}', token):
        raise ValueError('Invalid executor connection token')
    values = dict(fields or {})
    if 'method' in values or 'token' in values:
        raise ValueError('Reserved executor field')
    values.update(method=method, token=token)
    lines = []
    for key, value in values.items():
        value = str(value)
        if not re.fullmatch(r'[a-z_]+', key) or not re.fullmatch(r'[A-Za-z0-9_.-]+', value):
            raise ValueError('Invalid flat executor field')
        lines.append(key + '=' + value)
    raw = ('\n'.join(lines) + '\n').encode('ascii')
    if len(raw) > MAX_FRAME:
        raise ValueError('Executor request exceeds frame limit')
    return struct.pack('<I', len(raw)) + raw


def read_exact(stream, count):
    result = bytearray()
    while len(result) < count:
        block = stream.read(count - len(result))
        if not block:
            raise ConnectionError('Executor disconnected; query the operation before retrying')
        result.extend(block)
    return bytes(result)


class CELiveAddTransport:
    def __init__(self, pipe=None, token=None):
        self.pipe = pipe or os.environ.get('NIOH3_LIVE_ADD_PIPE', '')
        self.token = token or os.environ.get('NIOH3_LIVE_ADD_TOKEN', '')
        if not re.fullmatch(r'nioh3-live-add-[0-9a-f]{32}', self.pipe):
            raise ValueError('A local live-add CE connection has not been configured')

    def call(self, method, **fields):
        frame = encode_request(method, self.token, fields)
        # The local server closes idle/incomplete requests after its bounded
        # timeout. Disconnect after dispatch is deliberately not retried here.
        with NonblockingPipe('\\\\.\\pipe\\' + self.pipe) as stream:
            offset = 0
            while offset < len(frame):
                count = stream.write(frame[offset:])
                if not count:
                    raise ConnectionError('Executor request was interrupted')
                offset += count
            size = struct.unpack('<I', read_exact(stream, 4))[0]
            if not 0 < size <= MAX_FRAME:
                raise ValueError('Executor response exceeds frame limit')
            result = json.loads(read_exact(stream, size))
        if not isinstance(result, dict) or result.get('protocol') != 1:
            raise ValueError('Executor response protocol differs')
        if result.get('ok') is not True:
            raise RuntimeError(result.get('error', 'Executor rejected the request'))
        return result['result']
