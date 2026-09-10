"""PC v2.01 seed-scoped challenge-capacity override; never edits remaining count."""
from dataclasses import dataclass, replace
import struct
from .runtime_auxiliary_override import RuntimeAuxiliaryOverrideSession

CAPACITY_RVA = 0x1028E30
CAPACITY_SIGNATURE = bytes.fromhex('48 89 5C 24 08')

@dataclass(frozen=True)
class ChallengeOverrideProfile:
    seed: int
    capacity: int

    def __post_init__(self):
        if not 0 <= self.seed <= 0xFFFFFFFF or not 1 <= self.capacity <= 7:
            raise ValueError('Expected a uint32 seed and a capacity from 1 to 7')

def build_challenge_trampoline(profile, *, return_address, counter_address, original_instruction):
    # Verified getter passes seed in EDX and returns capacity in AL/EAX.
    # Preserve incoming flags and the original entry's RBX spill for other seeds.
    code = bytearray(b'\x9c\x81\xfa' + struct.pack('<I', profile.seed))
    # jnz normal; the matching path has popfq, push rax, mov rax, lock inc,
    # pop rax, mov eax, ret. Counter updates must preserve incoming flags too.
    match = b'\x50\x48\xb8' + struct.pack('<Q', counter_address) + b'\xf0\x48\xff\x00\x58\x9d\xb8' + struct.pack('<I', profile.capacity) + b'\xc3'
    code.extend(b'\x75' + bytes([len(match)]))
    code.extend(match)
    code.extend(b'\x9d' + original_instruction)
    # RIP-indirect tail jump preserves RAX as well as all other registers.
    code.extend(b'\xff\x25\x00\x00\x00\x00' + struct.pack('<Q', return_address))
    return bytes(code)

class RuntimeChallengeOverrideSession(RuntimeAuxiliaryOverrideSession):
    def __init__(self, profile, *, pid, runtime_profile):
        if runtime_profile.display_version != 'PC v2.01':
            raise ValueError('Challenge capacity override requires verified PC v2.01')
        super().__init__(profile, pid=pid, runtime_profile=replace(runtime_profile,
            descriptor_complete_rva=CAPACITY_RVA, descriptor_complete_signature=CAPACITY_SIGNATURE))

    def _build_code(self, profile, **kwargs):
        return build_challenge_trampoline(profile, **kwargs)

class OverrideGroup:
    """Retain all hook owners until every restoration is confirmed."""
    def __init__(self, sessions):
        self.sessions = sessions

    def start(self):
        for session in self.sessions:
            session.start()

    def stop(self):
        error = None
        for session in reversed(self.sessions):
            try:
                session.stop()
            except Exception as exception:
                error = exception
        if error:
            raise error

    def hit_count(self):
        return sum(session.hit_count() for session in self.sessions)
