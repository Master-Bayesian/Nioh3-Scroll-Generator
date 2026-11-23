"""Replay PC202 Water Sword's action-cost instructions under explicit actor conditions."""

import argparse
import hashlib
import json
from pathlib import Path
import struct

from unicorn import Uc, UC_ARCH_X86, UC_HOOK_CODE, UC_MODE_64
from unicorn import x86_const as reg


def num(blob, offset, fmt="I"):
    return struct.unpack_from("<" + fmt, blob, offset)[0]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def bits(value):
    return num(struct.pack("<f", value), 0)


def load(path, expected=None):
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if expected and digest != expected:
        raise ValueError(f"Unexpected PC202 input: {path}")
    return data


class Role:
    def __init__(self, path):
        self.path = path
        snap = json.loads((path / "snapshot.json").read_bytes())
        mem = json.loads((path / "ordinary-read-memory.json").read_bytes())
        if any(mem[k] != snap["process"][k] for k in ("pid", "creation_filetime")):
            raise ValueError("Role and memory come from different processes")
        self.process = snap["process"]
        self.roots = snap["roots"]
        self.inputs = snap["inputs"]
        self.regions = [
            (r["address"], bytes.fromhex(r["bytes"])) for r in mem["regions"]
        ]

    def read(self, address, length):
        matches = {
            raw[address - start : address - start + length]
            for start, raw in self.regions
            if start <= address and address + length <= start + len(raw)
        }
        if len(matches) > 1:
            raise ValueError(f"Conflicting retained bytes at {address:#x}")
        return next(iter(matches), None)

    def value(self, address, fmt):
        data = self.read(address, struct.calcsize("<" + fmt))
        return num(data, 0, fmt) if data is not None else None

    def actor_conditions(self):
        actor, owner = self.roots["player"], self.roots["owner"]
        if self.value(actor + 0x3A0, "Q") != owner:
            raise ValueError("The role has inconsistent actor and owner roots")
        descriptor = self.value(actor + 0xE8, "Q")
        controller = self.value(actor + 0x3B0, "Q")
        if descriptor is None or controller is None:
            raise ValueError("Missing root pointers")
        flags = self.value(descriptor + 0x68, "I") if descriptor else None
        member = self.value(controller + 0x40, "Q") if controller else 0
        mask = self.value(member, "B") if member else None
        return dict(
            actor=actor,
            owner=owner,
            descriptor=descriptor,
            controller=controller,
            descriptor_bit31=None if flags is None else bool(flags & (1 << 31)),
            controller_member=member,
            suppress_general_reduction=False
            if member == 0
            else None
            if mask is None
            else bool(mask & 4),
            status_cost_factors=[
                self.value(owner + off, "f") for off in (0x2E80, 0x2E84, 0x2E88)
            ],
        )


def skill_adjustments(directory, configured):
    defs = load(
        directory / "skills130.bin",
        "545e426ee6161feaac81d7374d43f5017f537e1605914bd16a8321c54443ed86",
    )
    fx = load(
        directory / "skills598.bin",
        "1fcb81c31677a7d6d082c8c9a719e29770cf372639e6c9e3ba7fcd3d55dc971b",
    )
    enabled = {s["key"]: s["rank"] for s in configured if s["enabled"]}
    values, definitions = {}, {}
    for key in (0x98CF, 0x2E74):
        (row,) = [
            r
            for i in range(8, len(defs), 192)
            if num(r := defs[i : i + 192], 0x18, "H") == key
        ]
        definitions[key] = row
        total = 0
        for offset in (0x34, 0x38, 0x3C):
            ref = num(row, offset, "H")
            if ref not in enabled:
                continue
            (effect,) = [
                r
                for i in range(8, len(fx), 148)
                if num(r := fx[i : i + 148], 0, "H") == ref
                and num(r, 4) == enabled[ref]
            ]
            total += sum(
                num(effect, at + 8)
                for at in (0x40, 0x4C, 0x58)
                if num(effect, at, "H") == 0xEDBC
            )
        values[key] = total
    return values, definitions


def same_role_memory_coverage(role):
    """Search retained ordinary reads by process identity and exact field address."""
    paths = sorted(role.path.parent.rglob("ordinary-read-memory.json"))
    facts = role.actor_conditions()
    wanted = {
        "descriptor_bit31_byte": (facts["descriptor"] + 0x6B, 1),
        "controller_member_pointer": (facts["controller"] + 0x40, 8),
        "status_factor_pair": (facts["owner"] + 0x2E80, 8),
        "status_factor_third": (facts["owner"] + 0x2E88, 4),
    }
    counts = dict.fromkeys(wanted, 0)
    same_process = 0
    for path in paths:
        doc = json.loads(path.read_bytes())
        if any(doc[k] != role.process[k] for k in ("pid", "creation_filetime")):
            continue
        same_process += 1
        for name, (at, size) in wanted.items():
            if any(
                r["address"] <= at and at + size <= r["address"] + len(r["bytes"]) // 2
                for r in doc["regions"]
            ):
                counts[name] += 1
    return dict(
        files_examined=len(paths),
        same_process=same_process,
        conflicts_not_resolved_across_time=True,
        files_covering_each_address=counts,
    )


class Native:
    # Rebind the call-frame objects to addresses in the emulator.
    actor, owner, state, action, descriptor, entity = (
        0x50000000 + n * 0x10000 for n in range(6)
    )
    record, controller, suppression, vtable, db, stat, materializer, stack = (
        0x50100000 + n * 0x10000 for n in range(8)
    )
    stop = 0x50190000
    global_root = 0x45B9E30

    def __init__(self, sections, role, scalars, skill, definitions, kind):
        self.vm = Uc(UC_ARCH_X86, UC_MODE_64)
        self.scalars, self.skills, self.kind = scalars, skill, kind
        self.adapters = set()
        self.definitions = {
            key: self.db + 0x1000 + i * 192 for i, key in enumerate(definitions)
        }
        for name, address, digest in (
            (
                "text",
                0x1000,
                "4cec8fb6ad867417a76df8201c1d4f54172443884910463c1953acad91ae6c29",
            ),
            (
                "rdata",
                0x38E2000,
                "112be451ab3b2abef3bd90c608bf7853c2607e96250894c82670fabbd74efa81",
            ),
        ):
            raw = load(sections / f"Nioh3_v2.0.2.0.{name}.bin", digest)
            self.vm.mem_map(address, (len(raw) + 4095) & ~4095)
            self.vm.mem_write(address, raw)
        self.vm.mem_map(0x45B9000, 0x2000)
        self.vm.mem_map(self.actor, 0x1A0000)
        for key, row in definitions.items():
            self.vm.mem_write(self.definitions[key], row)
        for address, value in (
            (self.global_root, self.db),
            (self.db + 0x230, self.db),
            (self.db + 0x8A8, self.db),
            (self.actor + 0xE8, self.descriptor),
            (self.actor + 0x3A0, self.owner),
            (self.actor + 0x3B0, self.controller),
            (self.controller + 8, self.state),
            (self.controller + 0x40, self.suppression),
            (self.state, self.vtable),
            (self.vtable + 0xC0, 0x2E9934),
            (self.vtable + 0x178, 0x555B00),
            (self.state + 0xE0, self.actor),
            (self.state + 0xE8, self.record),
            (self.record + 0x20, self.action),
        ):
            self.put(address, value, "Q")
        self.put(self.state + 0x85C, role.inputs["style"], "i")
        self.put(self.stat + 0x79C, role.value(role.roots["stat"] + 0x79C, "I"))
        self.put(self.owner + 0x2C, role.value(role.roots["owner"] + 0x2C, "I"))
        self.put(
            self.owner + 0x3670, role.value(role.roots["owner"] + 0x3670, "B"), "B"
        )
        self.vm.mem_write(
            self.materializer, role.read(role.roots["materializer"], 0x900)
        )
        # Original A66CC/A648C rebuild an empty aggregate to these identity factors.
        for offset in (0x2E80, 0x2E84, 0x2E88):
            self.put(self.owner + offset, 1.0, "f")
        for pc in (
            0x85B6C,
            0x85574,
            0x624518,
            0x608508,
            0x6084C0,
            0x608530,
            0x6084E0,
            0x478868,
            0xA1C254,
            0x555B00,
            0x55A41C,
            0xA5AF4,
            0x136A44,
            0x556E0C,
            0x1359FC,
        ):
            self.vm.hook_add(UC_HOOK_CODE, self.adapter, begin=pc, end=pc)
        self.vm.hook_add(UC_HOOK_CODE, self.sentinel, begin=self.stop, end=self.stop)

    def put(self, address, value, fmt="I"):
        self.vm.mem_write(address, struct.pack("<" + fmt, value))

    @staticmethod
    def sentinel(vm, _pc, _size, _data):
        vm.emu_stop()

    def adapter(self, vm, pc, _size, _data):
        self.adapters.add(hex(pc))
        key = vm.reg_read(reg.UC_X86_REG_EDX)
        if pc == 0x478868:
            if key != 255 or key in self.kind:
                raise ValueError(f"Expected an absent action-kind key, got {key}")
            value = 0
        elif pc == 0xA1C254:
            value = self.skills[vm.reg_read(reg.UC_X86_REG_ECX)]
        elif pc == 0x556E0C:
            value = self.definitions[key]
        elif pc == 0x608508:
            vm.reg_write(reg.UC_X86_REG_XMM0, bits(self.scalars[key]))
            value = 0
        elif pc == 0x6084C0:
            value = int(self.scalars[key])
        elif pc == 0x608530:
            value = self.db if key in self.scalars else 0
        elif pc == 0x6084E0:
            value = int(self.scalars[0x5A3B])
        elif pc in (0x85B6C, 0x85574):
            value = self.stat
        elif pc == 0x624518:
            value = self.materializer
        elif pc == 0x136A44:
            if key != 0x368:
                raise ValueError(f"Unexpected empty-status lookup {key}")
            value = 0
        elif pc == 0x1359FC:
            if key != 0x36D8:
                raise ValueError(f"Unexpected empty-status object {key}")
            value = 0
        elif pc in (0x555B00, 0x55A41C, 0xA5AF4):
            # Samurai form; zero D+1A0 makes either conditional reduction identical.
            # D+414/D+41C/D+41D are zero, so either weapon-special case is neutral.
            value = 0
        else:
            raise ValueError(f"Unbound helper {pc:#x}")
        vm.reg_write(reg.UC_X86_REG_RAX, value)
        sp = vm.reg_read(reg.UC_X86_REG_RSP)
        vm.reg_write(reg.UC_X86_REG_RIP, num(vm.mem_read(sp, 8), 0, "Q"))
        vm.reg_write(reg.UC_X86_REG_RSP, sp + 8)

    def execute(self, start, end, registers):
        for key, value in registers.items():
            self.vm.reg_write(getattr(reg, "UC_X86_REG_" + key), value)
        try:
            self.vm.emu_start(start, end, count=15000)
        except Exception as exc:
            pc = self.vm.reg_read(reg.UC_X86_REG_RIP)
            raise RuntimeError(
                f"PC202 replay stopped at {pc:#x} from {start:#x}"
            ) from exc
        if self.vm.reg_read(reg.UC_X86_REG_RIP) != end:
            raise RuntimeError("PC202 replay did not reach its declared exit")

    def call(self, entry, **registers):
        self.put(self.stack - 8, self.stop, "Q")
        self.execute(entry, self.stop, {"RSP": self.stack - 8, **registers})
        return self.vm.reg_read(reg.UC_X86_REG_AL)

    def cost(self, action, flag, suppress):
        self.vm.mem_write(self.action, action)
        self.put(self.descriptor + 0x68, int(flag) << 31)
        self.put(self.suppression, int(suppress) << 2, "B")
        base_cost = num(action, 0x16, "h")
        self.put(self.state + 0x100C, float(base_cost), "f")
        self.put(self.stack + 0x40, self.owner, "Q")
        self.execute(
            0x4F6A07,
            0x4F73E3,
            dict(
                RSP=self.stack,
                RBP=self.stack + 0x200,
                RDI=self.state,
                RSI=self.actor,
                R15=self.action,
                R12=self.owner + 0x2DC0,
                XMM10=bits(1.0),
                XMM11=bits(0.01),
                XMM12=0,
            ),
        )
        return num(self.vm.mem_read(self.state + 0x100C, 4), 0, "f")

    def admits(self, balance, cost):
        self.put(self.entity + 0x14, balance, "f")
        return self.call(0x49B108, RCX=self.entity, XMM1=bits(cost)) != 0

    def action_flags(self, action):
        self.vm.mem_write(self.action, action)
        return {
            hex(p): bool(self.call(0x2E9934, RCX=self.state, EDX=p))
            for p in (2, 0x11, 0x21, 0x2B, 0xAD, 0x104)
        }

    def ordinary_cost_route(self, action):
        self.vm.mem_write(self.action, action)
        alternative_gauge = self.call(0x377E90, RCX=self.record)
        alternative_mode = self.call(0x378554, RCX=self.state)
        action_flags = num(action, 0x110, "Q")
        # REA98 4F54C4, the only two later subtract-from-100C stores.
        later_discounts = {
            0x4D48,
            0x4F07,
            0x5CCF,
            0x6BFD,
            0xA78C,
            0xD1AA,
            0xAF4A,
            0xD606,
        }
        return (
            not alternative_gauge
            and not alternative_mode
            and not action_flags & ((1 << 33) | (1 << 29))
            and num(action, 0xDC, "H") not in later_discounts
        )


def run(root):
    deliverables = root / "deliverables"
    role = Role(deliverables / "optimizer-live-samurai-20261120/dual-product")
    if role.inputs["mode"] != 0 or role.inputs["status_keys"]:
        raise ValueError("Expected the retained Samurai empty-status observation")
    resource = load(
        deliverables
        / "event-graph-20261111/matched-resources/archive-01-entry-00091.bin",
        "fff9383f4d499a229d7e66cd000261ceb953dbd680830c4e3b57fd75dcf34f09",
    )
    at, length = struct.unpack_from("<II", resource, 12)
    actions = {
        num(r, 0xC, "H"): r
        for i in range(at, at + length, 336)
        if len(r := resource[i : i + 336]) == 336
    }
    ids = tuple(range(25329, 25336))
    ki = deliverables / "ki-write-live-20261113"
    rows = load(
        ki / "action-kind-3995.bin",
        "c2e92d30f1a5ce9acb8e1a16de5eef6bfa002af8bf566fffbb18df9a701abd7d",
    )
    kind = {rows[i + 16] for i in range(8, len(rows), 20)}
    scalars = {
        int(r["key"], 16): r["float"]
        for r in json.loads((ki / "COST_SCALAR_PARAMETERS.json").read_bytes())
    }
    skill, definitions = skill_adjustments(
        deliverables
        / "martial-event-live-20261111/phase06-reaper-complete/player-sources",
        role.inputs["configured_skills"],
    )
    sections = (
        root
        / "cache/original/nioh3-r10-pc202-20261016/input/r8/input/evidence/pro-r7-forward/evidence/original-sections"
    )
    native = Native(sections, role, scalars, skill, definitions, kind)
    remaining = role.value(role.roots["stat"] + 0x1C, "I")
    gates = {i: native.action_flags(actions[i]) for i in ids}
    ordinary_route = {i: native.ordinary_cost_route(actions[i]) for i in ids}
    if not all(ordinary_route.values()):
        raise ValueError(
            "An action takes another cost route before or after the ordinary calculation"
        )
    d = role.roots["materializer"]

    def read_factor(offset):
        return f32(1.0 - role.value(d + offset, "f"))

    if not (
        native.admits(1.0, 200.0)
        and not native.admits(0.0, 200.0)
        and not native.admits(-1.0, 70.0)
        and native.admits(0.0, 0.0)
    ):
        raise ValueError(
            "Native admission disagrees at the final-debit or zero-cost boundary"
        )
    matrix = []
    for flag in (False, True):
        for suppress in (False, True):
            costs = {i: native.cost(actions[i], flag, suppress) for i in ids}
            ratio = f32(
                f32((1.0 if suppress else read_factor(0x160)) * read_factor(0x7F0))
                * read_factor(0x170)
            )
            if flag:
                ratio = f32(ratio * scalars[0x254E])
            expected = {i: f32(num(actions[i], 0x16, "h") * ratio) for i in ids}
            if expected != costs:
                raise ValueError(
                    f"Native debit differs from the declared field model for actor flags {(flag, suppress)}"
                )
            if not (
                costs[25330] == costs[25332] == costs[25334]
                and costs[25331] == costs[25333] == costs[25335]
            ):
                raise ValueError(
                    "The ordinary and upgraded alternating loops have different costs"
                )
            sequence, balance = [], float(remaining)
            while len(sequence) < 100:
                i = 25329 if not sequence else (25330 if len(sequence) % 2 else 25331)
                if not native.admits(balance, costs[i]):
                    break
                sequence.append(i)
                balance = f32(balance - costs[i])
            if not sequence or len(sequence) == 100 or balance > 0:
                raise ValueError("The declared Ki scenario failed to stop")
            matrix.append(
                dict(
                    descriptor_bit31=flag,
                    suppress_general_reduction=suppress,
                    debit_per_action=costs,
                    action_count=len(sequence),
                    final_balance=balance,
                )
            )
    return dict(
        schema="pc202-conditional-water-sword-cost/v1",
        role=role.process,
        maximum_ki=remaining,
        skill_reductions_per_mille=skill,
        actor_observation=role.actor_conditions(),
        same_role_memory_coverage=same_role_memory_coverage(role),
        action_predicates=gates,
        ordinary_cost_route=ordinary_route,
        minimum_read=dict(
            descriptor_bit31="Read u64 at actor+E8, then bit31 of u32 at descriptor+68.",
            suppress_general_reduction="Read u64 at actor+3B0, then u64 at controller+40, then bit2 of u8 at member+0. A null controller or member makes this condition false.",
            status_cost_factors="actor+3A0 is the owner. Read three contiguous f32 at owner+2E80. Water Sword uses the first two while predicate 0x21 is false.",
            independent_charge="If collecting a Water Sword debit, retain the action ID and adjusted f32 at action-state+100C after 4F73DB for the same actor and loadout.",
            reused="Action rows, configured skills, materializer D, agility, empty status and maximum Ki are already present for the retained role.",
        ),
        base_costs={i: num(actions[i], 0x16, "h") for i in ids},
        scenarios=matrix,
        native_ranges=[
            "4F6A07..4F73E3",
            "49B108..49B11F",
            "2E9934/2E9948",
            "376B60",
            "976100",
            "13582C",
            "377E90",
            "378554",
        ],
        helper_adapters=sorted(native.adapters),
        evidence_scope=(
            "Conditional PC202 cost producer replay with explicit object and helper bindings. "
            "No Ki recovery, interruption or voluntary release. The final debit may exceed the balance. "
            "Counts are actions, not landed hits. No observed Water Sword cast or current branch selection."
        ),
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("F:/Nioh3_r16_build"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = run(args.root)
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "report": str(args.output),
                "actions": [r["action_count"] for r in result["scenarios"]],
            }
        )
    )
