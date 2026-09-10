"""Shared bounded framing; no application or role dependencies."""
import json
import struct

MAX_FRAME_BYTES = 4 * 1024 * 1024

def read_exact(stream, size):
    chunks = bytearray()
    while len(chunks) < size:
        part = stream.read(size - len(chunks))
        if not part:
            raise EOFError('Truncated frame')
        chunks.extend(part)
    return bytes(chunks)


def read_frame(stream):
    first = stream.read(1)
    if not first:
        return None
    size = struct.unpack('<I', first + read_exact(stream, 3))[0]
    if not 0 < size <= MAX_FRAME_BYTES:
        raise ValueError('Frame size exceeds protocol limit')
    return json.loads(read_exact(stream, size).decode('utf-8'))


def write_frame(stream, payload):
    raw = json.dumps(payload, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode('utf-8')
    if len(raw) > MAX_FRAME_BYTES:
        raise ValueError('Response exceeds protocol limit')
    stream.write(struct.pack('<I', len(raw)) + raw)
    stream.flush()
