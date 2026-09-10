"""Python emitter for the previously accepted x64 mission-thread shim.

This is an executor implementation, not a generator or an arbitrary-code API.
Keep its emitted bytes compatible with build_dispatch_probe_code.lua.
"""
import struct


def build_dispatch_code(memory, resume, original, leaf=None, argument=0,
                        second_argument=None, insertion=None, preserve_rarity5=False):
    code = bytearray()
    jumps = []

    def emit(value):
        code.extend(bytes.fromhex(value))

    def u64(value):
        code.extend(struct.pack('<Q', value))

    def marker(offset, value):
        displacement = offset - (len(code) + 10)
        emit('C7 05')
        code.extend(struct.pack('<iI', displacement, value))

    def reject_unless_equal():
        emit('0F 85')
        jumps.append(len(code))
        code.extend(bytes(4))

    if leaf is not None:
        emit('9C 50 51 52 41 50 41 51 41 52 41 53')
        stack_size, xmm_offset = (0x98, 0x30) if insertion else (0x88, 0x20)
        emit('48 81 EC')
        code.extend(struct.pack('<I', stack_size))
        def xmm(opcode):
            for i in range(6):
                offset = xmm_offset + i * 16
                code.extend(bytes((0xF3, 0x0F, opcode, (0x44 if offset < 128 else 0x84) + i * 8, 0x24)))
                code.extend(bytes((offset,)) if offset < 128 else struct.pack('<I', offset))
        xmm(0x7F)
        emit('48 B9'); u64(argument)
        emit('48 B8'); u64(leaf)
        if second_argument is not None:
            emit('48 BA'); u64(second_argument)
        else:
            emit('33 D2')
        emit('FF D0 48 A3'); u64(memory + 0x308)
        if preserve_rarity5:
            # Match NativeBatchOracle's v2.01 raw-R5 header preservation.
            # Only our returned scratch record is touched, before insertion.
            emit('49 BA'); u64(memory + 0x600)
            emit('4C 39 D0'); reject_unless_equal()
            emit('66 81 78 30 04 04 75 06 66 C7 40 30 05 05')
            emit('66 81 78 30 05 05'); reject_unless_equal()
        if insertion:
            marker(0x318, 1)
            emit('49 BA'); u64(memory + 0x600)
            emit('4C 39 D0'); reject_unless_equal()
            emit('49 BB'); u64(insertion['serial'])
            emit('4D 39 5A 28'); reject_unless_equal()
            emit('49 BA'); u64(insertion['data'] + insertion.get('serial_counter_offset', 8))
            emit('49 BB'); u64(insertion['serial'] + 1)
            emit('4D 39 1A'); reject_unless_equal()
            marker(0x318, 2)
            emit('48 B9'); u64(insertion['manager'])
            emit('48 BA'); u64(memory + 0x800)
            emit('49 B8'); u64(memory + 0x600)
            emit('49 B9'); u64(memory + 0x320)
            emit('C7 44 24 20 00 00 00 00 48 B8'); u64(insertion['function_address'])
            emit('FF D0 48 A3'); u64(memory + 0x328)
            marker(0x318, 3)
        for offset in jumps:
            struct.pack_into('<i', code, offset, len(code) - offset - 4)
        xmm(0x6F)
        emit('48 81 C4'); code.extend(struct.pack('<I', stack_size))
        emit('41 5B 41 5A 41 59 41 58 5A 59 58 9D')
    marker(0x300, 1)
    code.extend(original)
    emit('FF 25 00 00 00 00'); u64(resume)
    if len(code) >= 0x300:
        raise ValueError('Dispatch code overlaps its data')
    return bytes(code)
