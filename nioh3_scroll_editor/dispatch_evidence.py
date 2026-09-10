"""Offline verification of preserved execution context."""
REGISTERS = ('RAX', 'RBX', 'RCX', 'RDX', 'RSI', 'RDI', 'RBP',
             'R8', 'R9', 'R10', 'R11', 'R12', 'R13', 'R14', 'R15')


def verify_dispatch(execution):
    if execution.get('phase') != 'completed' or execution.get('redirect_count') != 1:
        raise ValueError('Exactly one acknowledged redirect is required')
    if execution.get('released') is not True or execution.get('breakpoints') != []:
        raise ValueError('Allocation or breakpoint cleanup is not confirmed')
    before, after = execution['before'], execution['after']
    if any(before[key] != after[key] for key in REGISTERS):
        raise ValueError('A general register changed')
    if before['RSP'] % 16 != 8 or after['RSP'] != before['RSP'] - 0x48:
        raise ValueError('Unexpected stack alignment or prologue delta')
    if after['RIP'] != before['RIP'] + 7:
        raise ValueError('Continuation is not the end of the replayed prologue')
    # push rbx; push rdi; sub rsp,0x38. Only SUB changes arithmetic flags.
    left = before['RSP'] - 16
    result = (left - 0x38) & ((1 << 64) - 1)
    expected = ((left < 0x38) | (((result & 255).bit_count() % 2 == 0) << 2)
                | ((((left ^ 0x38 ^ result) & 16) != 0) << 4)
                | ((result == 0) << 6) | (((result >> 63) & 1) << 7)
                | ((((left ^ 0x38) & (left ^ result) & (1 << 63)) != 0) << 11))
    if after['EFLAGS'] & 0x8D5 != expected:
        raise ValueError('Arithmetic flags do not match the original prologue')
    # Debugger RF/TF are not product flags and may differ at hardware stops.
    if (before['EFLAGS'] ^ after['EFLAGS']) & ~(0x8D5 | 0x10100):
        raise ValueError('Non-arithmetic flags changed')
