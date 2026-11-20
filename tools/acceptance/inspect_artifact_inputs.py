"""Independent, read-only inspection of final handoff bytes (no Windows required)."""
from __future__ import annotations

import argparse
import ctypes
import hashlib
import io
import json
from pathlib import Path
import re
import struct
import zipfile


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sections(data: bytes) -> dict[str, bytes]:
    result = {}
    at = 0
    while at < len(data):
        size = struct.unpack_from('<I', data, at)[0]
        at += 4
        name = data[at:at + size].decode('utf-8')
        at += size
        size = struct.unpack_from('<I', data, at)[0]
        at += 4
        if name in result or at + size > len(data):
            raise ValueError('invalid frame section')
        result[name] = data[at:at + size]
        at += size
    return result


def pe_exports(data: bytes) -> dict:
    """Parse PE export addresses and decode literal ABI query return values."""
    pe = struct.unpack_from('<I', data, 0x3c)[0]
    if data[pe:pe + 4] != b'PE\0\0':
        raise ValueError('not PE')
    machine, count = struct.unpack_from('<HH', data, pe + 4)
    optional_size = struct.unpack_from('<H', data, pe + 20)[0]
    optional = pe + 24
    if struct.unpack_from('<H', data, optional)[0] != 0x20b:
        raise ValueError('expected PE32+')
    table = optional + optional_size
    image_base = struct.unpack_from('<Q', data, optional + 24)[0]
    mappings = []
    for index in range(count):
        start = table + index * 40
        virtual_size, rva, raw_size, raw_offset = struct.unpack_from('<IIII', data, start + 8)
        mappings.append((rva, max(virtual_size, raw_size), raw_offset))

    def offset(rva):
        for base, length, raw in mappings:
            if base <= rva < base + length:
                return raw + rva - base
        raise ValueError(f'RVA out of image: {rva:x}')

    def string(rva):
        at = offset(rva)
        end = data.index(0, at)
        return data[at:end].decode('ascii')

    export_rva, export_size = struct.unpack_from('<II', data, optional + 112)
    start = offset(export_rva)
    _, _, _, _, _, ordinal_base, functions, names, funcs, name_ptrs, ordinals = struct.unpack_from('<IIHHIIIIIII', data, start)
    exports = {}
    for index in range(names):
        name = string(struct.unpack_from('<I', data, offset(name_ptrs) + 4 * index)[0])
        ordinal = struct.unpack_from('<H', data, offset(ordinals) + 2 * index)[0]
        if ordinal >= functions:
            raise ValueError('invalid export ordinal')
        rva = struct.unpack_from('<I', data, offset(funcs) + 4 * ordinal)[0]
        entry = {'rva': hex(rva), 'ordinal': ordinal_base + ordinal}
        if export_rva <= rva < export_rva + export_size:
            entry['forwarder'] = string(rva)
        else:
            code = data[offset(rva):offset(rva) + 16]
            entry['first_16_bytes'] = code.hex()
            if code[0:1] == b'\xb8' and code[5:6] == b'\xc3':
                entry['literal_return_u32'] = struct.unpack_from('<I', code, 1)[0]
        exports[name] = entry
    return {'machine': hex(machine), 'image_base': hex(image_base), 'exports': exports}


def inspect(handoff: Path) -> dict:
    exe = (handoff / 'app/Nioh3Studio-0.8.13-win-x64.exe').read_bytes()
    magic, size, expected = struct.unpack('<16sQ32s', exe[-56:])
    if magic != b'NIOH3_ONEFILE_V1':
        raise ValueError('onefile footer missing')
    payload = exe[-56-size:-56]
    if hashlib.sha256(payload).digest() != expected:
        raise ValueError('payload hash differs')
    scene_prefix = 'worker/runtime/data/loadout/kusarigama-strong/'
    with zipfile.ZipFile(io.BytesIO(payload)) as archive:
        manifest = json.loads(archive.read('build-manifest.json'))
        members = archive.namelist()
        if len(members) != len(set(members)):
            raise ValueError('duplicate ZIP members')
        manifest_paths = set()
        for item in manifest['files']:
            raw = archive.read(item['path'])
            if len(raw) != item['size'] or digest(raw) != item['sha256']:
                raise ValueError('embedded member identity differs: ' + item['path'])
            manifest_paths.add(item['path'])
        unlisted = set(members) - manifest_paths - {'build-manifest.json'}
        if unlisted:
            raise ValueError('unmanifested members: ' + ', '.join(sorted(unlisted)))
        scene = {name[len(scene_prefix):]: archive.read(name) for name in members if name.startswith(scene_prefix)}
        cpu = scene['main_cpu.dll']
        descriptor = json.loads(scene['bundle.json'])
        packaged_sections = sections(scene['frame.bin'])
        packaged_addresses = struct.unpack_from('<9Q', packaged_sections['addresses'])
        native_root = handoff / 'native'
        tested_cpu = (native_root / 'host-tested/main_cpu.dll').read_bytes()
        tested_cuda = (native_root / 'host-tested/main_device.dll').read_bytes()
        host_bundle = handoff / 'replay-inputs/bundle'
        host_sections = sections((host_bundle / 'frame.bin').read_bytes())
        host_addresses = struct.unpack_from('<9Q', host_sections['addresses'])
        generated = (native_root / 'project-source/prepared-main/main_gen.inc').read_text()
        constants = {name: int(value, 16) for name, value in re.findall(r'^#define (MAIN_\w+) (0x[0-9a-fA-F]+)ull', generated, re.M)}
        entry = (host_bundle / 'context/scene_entry.bin').read_bytes()
        registers = struct.unpack_from('<16Q', entry, 16)
        return {
            'schema': 'independent-artifact-input-inspection/v1',
            'environment': 'Linux read-only PE/ZIP/scene inspection; no Windows or GPU execution',
            'source_commit': manifest['git']['commit'],
            'outer_sha256': digest(exe), 'payload_sha256': digest(payload),
            'manifest_members_verified': len(manifest_paths), 'zip_members': len(members),
            'packaged_scenes': sorted({n.split('/')[4] for n in members if n.startswith('worker/runtime/data/loadout/')}),
            'scene_descriptor': descriptor,
            'scene_files': {name: {'bytes': len(raw), 'sha256': digest(raw)} for name, raw in scene.items()},
            'main_device_paths': [n for n in members if n.endswith('/main_device.dll')],
            'dense_kernel_paths': [n for n in members if n.endswith('/kernel.dll')],
            'context_json_paths': [n for n in members if n.startswith('worker/runtime/data/loadout/') and n.endswith('/context.json')],
            'packaged_cpu': {'sha256': digest(cpu), **pe_exports(cpu)},
            'tested_cpu': {'sha256': digest(tested_cpu), **pe_exports(tested_cpu)},
            'tested_cuda': {'sha256': digest(tested_cuda), **pe_exports(tested_cuda)},
            'packaged_cpu_equals_tested': cpu == tested_cpu,
            'packaged_frame_sections': {name: len(raw) for name, raw in packaged_sections.items()},
            'tested_frame_sections': {name: len(raw) for name, raw in host_sections.items()},
            'packaged_frame_addresses': [hex(x) for x in packaged_addresses],
            'tested_frame_addresses': [hex(x) for x in host_addresses],
            'compiled_constants': {name: hex(value) for name, value in constants.items()},
            'tested_scene_entry': {'rcx': hex(registers[1]), 'rsp': hex(registers[4])},
            'tested_frame_matches_compiled_main': host_addresses[2] == constants['MAIN_SCORE'] - 0x218,
            'packaged_frame_matches_compiled_main': packaged_addresses[2] == constants['MAIN_SCORE'] - 0x218,
            'whole_scene_replacement_required': packaged_addresses != host_addresses,
            'scope': 'Byte identity and compatibility rejection only. Does not certify any player score.',
        }


def native_mixed_scene_control(handoff: Path, library: Path, report: dict) -> dict:
    """Exercise actual translated CPU loader with a deliberate wrong-scene RCX.

    Uses one synthetic zero memory chunk. No private memory image is present.
    The loader rejects before executing MAIN; numerical acceptance is not claimed.
    """
    class File(ctypes.Structure):
        _fields_ = [('name', ctypes.c_char_p), ('data', ctypes.c_void_p), ('size', ctypes.c_uint64)]

    native = ctypes.CDLL(str(library.resolve()))
    native.nk_context_abi.restype = ctypes.c_uint32
    native.nk_main_cpu_components_abi.restype = ctypes.c_uint32
    native.nk_main_cpu_open.argtypes = [ctypes.POINTER(File), ctypes.c_uint32, ctypes.c_char_p, ctypes.c_uint32]
    native.nk_main_cpu_open.restype = ctypes.c_void_p
    native.nk_main_cpu_close.argtypes = [ctypes.c_void_p]
    entry = bytearray((handoff / 'replay-inputs/bundle/context/scene_entry.bin').read_bytes())
    struct.pack_into('<Q', entry, 24, int(report['packaged_frame_addresses'][2], 16))
    raws = [bytes(8 + 65536), bytes(entry), bytes(8 + 8192), b'NKSCENE1' + bytes(8)]
    names = [b'image.bin', b'scene_entry.bin', b'image_known.bin', b'scene_metadata.bin']
    buffers = [ctypes.create_string_buffer(raw) for raw in raws]
    files = (File * 4)(*[File(name, ctypes.cast(buf, ctypes.c_void_p), len(raw)) for name, buf, raw in zip(names, buffers, raws)])
    error = ctypes.create_string_buffer(4096)
    handle = native.nk_main_cpu_open(files, 4, error, len(error))
    if handle:
        native.nk_main_cpu_close(handle)
        raise ValueError('mixed-scene loader control unexpectedly opened')
    expected = 'scene_entry.bin RCX/RSP do not match the compiled canonical MAIN/stack'
    message = error.value.decode('utf-8')
    if message != expected:
        raise ValueError('unexpected loader failure: ' + message)
    return {'kind': 'real Linux translated loader, synthetic wrong-scene entry', 'library_sha256': digest(library.read_bytes()),
            'context_abi': native.nk_context_abi(), 'components_abi': native.nk_main_cpu_components_abi(),
            'rejected': True, 'error': message, 'MAIN_executed': False}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('handoff', type=Path)
    parser.add_argument('--library', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    report = inspect(args.handoff)
    if args.library:
        report['mixed_scene_control'] = native_mixed_scene_control(args.handoff, args.library, report)
    text = json.dumps(report, indent=2, ensure_ascii=False) + '\n'
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding='utf-8')
    else:
        print(text)


if __name__ == '__main__':
    main()
