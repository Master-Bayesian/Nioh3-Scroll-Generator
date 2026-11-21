"""Materialize a newly observed opponent as a separate PC202 benchmark in a private replay bundle."""

import argparse
from bisect import bisect_right
import hashlib
import json
from itertools import groupby
from tempfile import TemporaryDirectory
from pathlib import Path
import shutil
import struct


def read_json(path):
    return json.loads(path.read_bytes())


class Unobserved(Exception):
    """A semantic operand has no recorded bytes."""


class Observation:
    def __init__(self, document):
        self.regions = sorted(
            (r["address"], bytes.fromhex(r["bytes"])) for r in document["regions"]
        )
        self.starts = [a for a, _ in self.regions]

    def read(self, address, length):
        index = bisect_right(self.starts, address) - 1
        if index >= 0:
            start, value = self.regions[index]
            offset = address - start
            if offset + length <= len(value):
                return value[offset : offset + length]
        raise Unobserved(f"{address:#x}+{length}")

    def ptr(self, address):
        return int.from_bytes(self.read(address, 8), "little")


class TargetImage:
    def __init__(self, observation, snapshot, opponent, canonical):
        self.mem = observation
        self.opponent = opponent
        self.canonical = canonical
        self.module = snapshot["module_base"]
        self.ranges = []
        roots = snapshot["roots"]
        for key, source, length in (
            ("source_owner", "owner", 0x4000),
            ("source_actor", "player", 0x1600),
            ("stat", "stat", 0x900),
            ("source", "source", 0x6000),
            ("d", "materializer", 0xB00),
        ):
            self.ranges.append((roots[source], canonical[key], length))
        self.bytes = {}
        self.missing = set()

    def relocate(self, value):
        if value == 0 or value == 2**64 - 1:
            return value
        if self.module <= value < self.module + 0x5000000:
            return self.canonical["module"] + value - self.module
        for a, dest, size in self.ranges:
            if a <= value < a + size:
                return dest + value - a
        moved = 0x500000000000 + value - self.opponent["actor"]
        if not 0x400000000000 <= moved < 0x600000000000:
            raise ValueError(f"Unbound non-heap pointer {value:#x}")
        return moved

    def store(self, at, data):
        for i, b in enumerate(data):
            if self.bytes.setdefault(at + i, b) != b:
                raise ValueError(f"Conflicting target operands at {at + i:#x}")

    def scalar(self, a, n):
        data = self.mem.read(a, n)
        self.store(self.relocate(a), data)
        return int.from_bytes(data, "little")

    def pointer(self, a):
        value = self.mem.ptr(a)
        self.store(self.relocate(a), struct.pack("<Q", self.relocate(value)))
        return value

    def link_resource(self, obj, index):
        if index == -1:
            return
        handle = self.pointer(obj + 0x48)
        if not handle:
            return
        entity = self.pointer(handle + 0x340)
        if not entity:
            return
        owner = self.pointer(entity + 0x3A0)
        if not owner:
            return
        subject = self.pointer(owner + 0x33F8)
        if not subject or self.scalar(subject + 0xAB84, 4) != 0xFFFFFFFF:
            return
        kind = self.pointer(subject + 0xB0)
        if kind and self.scalar(kind + 0x10, 4):
            self.scalar(owner + 0x33F8 + 8 + 12 * index, 4)

    def build(self, fields):
        o = self.opponent
        # Traverse actual paths. Addresses of old target-owned objects are never copied.
        for f in fields:
            root = {
                "target_actor": "actor",
                "target_owner": "owner",
                "target_stat": "stat",
            }.get(f["root"])
            if root is None or (root == "owner" and f["path"][0] == 0x33B0):
                continue
            if (
                root == "actor"
                and f["path"][:4] == [0x3B0, 8, 0xE8, 0x40]
                and len(f["path"]) > 4
            ):
                continue
            a = o[root]
            try:
                for off in f["path"][:-1]:
                    a = self.pointer(a + off)
                    if a == 0:
                        break
                if a:
                    address = a + f["path"][-1]
                    if f["kind"] in ("pointer", "module_pointer"):
                        self.pointer(address)
                    elif f["kind"] == "benchmark":
                        self.scalar(address, f["n"])
                    else:
                        raise ValueError(f"Unrecognized target operand {f['kind']}")
            except Unobserved as error:
                self.missing.add(str(error))
        ptr = self.pointer(o["actor"] + 0x3B0)
        if ptr:
            ptr = self.pointer(ptr + 8)
        if ptr:
            ptr = self.pointer(ptr + 0xE8)
        if ptr:
            parameter = self.pointer(ptr + 0x20)
            if parameter:
                self.scalar(parameter, 336)
            first = self.scalar(ptr + 0x48, 2)
            count = self.scalar(ptr + 0x4A, 2)
            if count:
                table = self.pointer(ptr + 0x40)
                for index in range(first, first + count):
                    row = self.pointer(table + 8 * index)
                    if row:
                        self.scalar(row + 0x72, 2)
                        try:
                            self.scalar(row, 192)
                        except Unobserved as error:
                            self.missing.add(str(error))
        # Only numeric target STAT. The pointer fields follow the typed paths above.
        for start, end in (
            (0x18, 0x44),
            (0x1C0, 0x222),
            (0x224, 0x798),
            (0x798, 0x7BC),
        ):
            self.scalar(o["stat"] + start, end - start)
        for off in (0x2EBC, 0x2EC0, 0x2EC4, 0x2F14, 0x2FA4, 0x2FA8, 0x2FAC, 0x2FFC):
            self.scalar(o["owner"] + off, 4)
        sent = self.pointer(o["owner"] + 0x33B0)
        if sent != o["sentinel"]:
            raise ValueError("Status sentinel differs from the double read")
        node = self.pointer(sent)
        for entry in o["objects"]:
            if node != entry["node"]:
                raise ValueError("Status list order differs")
            obj = self.pointer(node + 0x18)
            if obj != entry["address"]:
                raise ValueError("Status object moved")
            for off in (0, 0x28, 0x2C, 0x40, 0x54, 0x84):
                self.scalar(obj + off, 4)
            self.link_resource(obj, entry["resource_index"])
            node = self.pointer(node)
        if node != sent:
            raise ValueError("Incomplete status list")


def materialize(role, reference, recipe, output, target_module=None):
    if output.exists():
        raise ValueError("Choose a fresh output directory")
    receipt = read_json(reference / "receipt.json")
    if not receipt["ready"] or not receipt["replay_saved"]:
        raise ValueError("Reference capture incomplete")
    ref = read_json(reference / "reference-target.json")
    recorded = read_json(reference / "ordinary-read-memory.json")
    src = read_json(role / "snapshot.json")
    other = read_json(reference / "snapshot.json")
    if (
        any(src[k] != other[k] for k in ("process", "roots", "inputs", "module_base"))
        or recorded["pid"] != src["process"]["pid"]
        or recorded["creation_filetime"] != src["process"]["creation_filetime"]
        or ref["process"] != src["process"]
        or src["roots"]["player"] != ref["actor"]
    ):
        raise ValueError("Candidate role and target observation differ")
    schema = read_json(recipe)
    mem = Observation(recorded)
    image = TargetImage(mem, src, ref["opponent"], schema["roots"])
    if target_module is not None:
        if receipt["scope"] != "retained memory":
            raise ValueError("Module override is restricted to controlled replay")
        image.module = target_module
    image.build(schema["fields"])
    bundle = role / "bundle"
    ctx = bundle / "context"
    raw = bytearray((ctx / "image.bin").read_bytes())
    bits = bytearray((ctx / "image_known.bin").read_bytes())
    slots = {
        struct.unpack_from("<Q", raw, i)[0]: i // 65544
        for i in range(0, len(raw), 65544)
    }

    def write(a, b, replace=False):
        pg = a & ~65535
        o = a % 65536
        if pg not in slots:
            slots[pg] = len(slots)
            raw.extend(struct.pack("<Q", pg) + bytes(65536))
            bits.extend(struct.pack("<Q", pg) + bytes(8192))
        ix = slots[pg]
        pos = ix * 65544 + 8 + o
        mask = ix * 8200 + 8 + o // 8
        if not replace and bits[mask] & (1 << (o % 8)) and raw[pos] != b:
            raise ValueError("Target overlaps a conflicting source or ABI operand")
        raw[pos] = b
        bits[mask] |= 1 << (o % 8)

    for a, b in image.bytes.items():
        write(a, b)
    packet = schema["roots"]["packet"]
    for i, b in enumerate(struct.pack("<Q", image.relocate(ref["opponent"]["actor"]))):
        write(packet + 0x1C8 + i, b, True)
    manifest = read_json(ctx / "context.json")
    manifest["scenario"].update(
        image_sha256=hashlib.sha256(raw).hexdigest(),
        image_known_sha256=hashlib.sha256(bits).hexdigest(),
    )
    spans = []
    for _, g in groupby(enumerate(sorted(image.bytes)), lambda pair: pair[1] - pair[0]):
        addresses = [address for _, address in g]
        if not (0x400000000000 <= addresses[0] <= addresses[-1] < 0x600000000000):
            raise ValueError(
                "Opponent profile requires an active player field; cannot package it as fixed"
            )
        body = bytes(image.bytes[a] for a in addresses)
        spans.append(struct.pack("<QI", addresses[0], len(body)) + body)
    target = image.relocate(ref["opponent"]["actor"])
    packed = struct.pack(
        "<8sQQQI", b"N3FOE001", packet, schema["roots"]["module"], target, len(spans)
    ) + b"".join(spans)
    target_hash = hashlib.sha256(packed).hexdigest()
    scope = dict(
        reference_input=str(reference),
        scope="Controlled replay"
        if target_module is not None
        else "Separate observed opponent benchmark",
        optional_unobserved=sorted(image.missing),
        target_bytes=len(image.bytes),
        benchmark_target_sha256=target_hash,
        capture_source_sha256=hashlib.sha256(
            (reference / "reference-target.json").read_bytes()
        ).hexdigest(),
    )
    with TemporaryDirectory(dir=output.parent) as tmp:
        staged = Path(tmp) / "bundle"
        shutil.copytree(bundle, staged)
        (staged / "context/image.bin").write_bytes(raw)
        (staged / "context/image_known.bin").write_bytes(bits)
        (staged / "context/context.json").write_text(
            json.dumps(manifest), encoding="utf8"
        )
        (staged / "benchmark_target.bin").write_bytes(packed)
        descriptor = read_json(staged / "bundle.json")
        descriptor["benchmark_target_sha256"] = target_hash
        (staged / "bundle.json").write_text(
            json.dumps(descriptor, ensure_ascii=False, indent=2), encoding="utf8"
        )
        (staged / "TARGET_BENCHMARK.json").write_text(
            json.dumps(scope, indent=2), encoding="utf8"
        )
        staged.rename(output)
    return scope


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("role", "reference", "recipe", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument(
        "--target-module",
        type=lambda v: int(v, 0),
        help="Module base of a separately recorded opponent in a labeled replay control",
    )
    materialize(**vars(parser.parse_args()))
