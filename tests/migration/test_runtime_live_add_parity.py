"""Cross-language gate for the M3 inventory, descriptor and live-add slices.

The shipped implementation is the contract. Three things are compared with it on
the same inputs:

- the assembly descriptor and the assembly record, byte for byte, because those
  bytes are the native builder's input;
- the read-only inventory capture over one synthetic container, field by field,
  because that capture is the gate a count edit or a live addition trusts;
- the live-add and batch receipt state machines, step by step, through the real
  `LiveAddApplication` with an injected executor and through the Rust
  application with the equivalent injected executor, including the fault cases:
  a lost native reply, a proven pre-dispatch rejection, a missed idle window, a
  changed readback, a recycled process instance and both batch cancel
  boundaries.

No Nioh 3 process is touched. Every scenario is injected.
"""
from __future__ import annotations

import hashlib
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nioh3_scroll_editor import live_inventory  # noqa: E402
from nioh3_scroll_editor.candidate_transfer import import_candidate  # noqa: E402
from nioh3_scroll_editor.dispatch_evidence import verify_dispatch  # noqa: E402
from nioh3_scroll_editor.live_add_application import LiveAddApplication  # noqa: E402
from nioh3_scroll_editor.live_add_batch import LiveAddBatch  # noqa: E402
from nioh3_scroll_editor.live_add_descriptor import (  # noqa: E402
    assembly_descriptor,
    new_assembly_record,
    verify_assembly_preview,
)
from nioh3_scroll_editor.live_add_profile import PC_V201 as LAYOUT  # noqa: E402
from nioh3_scroll_editor.savegame import SCROLL_GROUP_OFFSET  # noqa: E402
from nioh3_scroll_editor.search_application import require_search_candidate_ready  # noqa: E402
from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402

CRATE = ROOT / "crates" / "nioh3-runtime"
PROBE = "runtime_read_probe"
RECORD_SIZE = 0xE8
CAPACITY = 400
CREATION = 134338049984156850
CREATION_LATER = 134338049984156851
BASE = 0x7FF000000000
MANAGER = BASE + 0x10000
DATA = BASE + 0x20000
CONTAINER = DATA + LAYOUT.container_offset
CONTEXT_DIGEST = "0f1e2d3c4b5a69788796a5b4c3d2e1f0"
FIXTURE_PID = 4321
PROFILE_ID = "pc-v2.01-live-add-r1"

MUTATION_SOURCES = (
    "mutation/memory.rs",
    "mutation/session.rs",
    "mutation/count.rs",
    "mutation/trampoline.rs",
    "mutation/inventory.rs",
    "mutation/descriptor.rs",
    "mutation/evidence.rs",
    "mutation/operations.rs",
    "mutation/live_add.rs",
    "mutation/live_batch.rs",
    "mutation/live_fakes.rs",
)
FORBIDDEN_EVERYWHERE = (
    "PROCESS_ALL_ACCESS",
    "0x1F0FFF",
    "0x1f0fff",
    "CreateRemoteThread",
    "TerminateProcess",
    "TerminateThread",
    "SuspendThread",
    "ResumeThread",
    "SetThreadContext",
    "DebugActiveProcess",
    "CreateProcessW",
    "OpenProcessToken",
)


def acceptance_target_dir() -> str:
    """An explicit acceptance cache wins; otherwise the shared resolver."""

    explicit = os.environ.get("NIOH3_RUNTIME_MUTATION_CARGO_TARGET", "").strip()
    if explicit:
        return explicit
    return resolved_cargo_target_dir("m3-runtime-acceptance")


def build_probe() -> Path:
    cargo = shutil.which("cargo")
    if cargo is None:
        raise AssertionError("cargo is required to build the runtime probe")
    target = acceptance_target_dir()
    completed = subprocess.run(
        [
            cargo,
            "build",
            "--locked",
            "--offline",
            "--quiet",
            "--features",
            "test-fake",
            "--manifest-path",
            str(CRATE / "Cargo.toml"),
            "--example",
            PROBE,
        ],
        cwd=str(ROOT),
        env={**os.environ, "CARGO_TARGET_DIR": target},
        capture_output=True,
        text=True,
        timeout=3600,
        check=False,
    )
    if completed.returncode != 0:
        raise AssertionError(
            "the runtime probe did not build: "
            + (completed.stderr.strip() or completed.stdout.strip())[-4000:]
        )
    binary = Path(target) / "debug" / "examples" / f"{PROBE}.exe"
    if not binary.is_file():
        raise AssertionError(f"the runtime probe is missing at {binary}")
    return binary


def fixture_record(seed: int = 0x0BAD, rarity: int = 4, record_type: int = 0x1E82) -> bytes:
    """One canonical record in the shape the assembly gate accepts."""

    record = bytearray(RECORD_SIZE)
    struct.pack_into("<H", record, 0, record_type)
    struct.pack_into("<H", record, 2, 0x0011)
    struct.pack_into("<H", record, 0x10, 0x2222)
    struct.pack_into("<I", record, 0x14, 0x33334444)
    struct.pack_into("<I", record, 0x18, 0x02800002)
    struct.pack_into("<I", record, 0x20, seed)
    struct.pack_into("<Q", record, 0x28, 0xFFFFFFFFFFFFFFFF)
    record[0x30] = rarity
    record[0x33] = 2
    struct.pack_into("<I", record, 0xDC, 0x55556666)
    for offset in range(0x34, 0xDC):
        record[offset] = (offset * 7 + 3) & 0xFF
    return bytes(record)


def container_bytes(records: list[tuple[int, int, int]], acquisition_order: int = 0) -> bytes:
    container = bytearray(CAPACITY * RECORD_SIZE)
    for slot, serial, seed in records:
        record = bytearray(fixture_record(seed=seed))
        struct.pack_into("<Q", record, 0x28, serial)
        struct.pack_into("<I", record, 0x1C, acquisition_order)
        container[slot * RECORD_SIZE : (slot + 1) * RECORD_SIZE] = record
    return bytes(container)


def save_bytes(container: bytes) -> bytes:
    saved = bytearray(SCROLL_GROUP_OFFSET + CAPACITY * RECORD_SIZE)
    for slot in range(CAPACITY):
        start = slot * RECORD_SIZE
        record = container[start : start + RECORD_SIZE]
        if record[:2] == b"\0\0":
            continue
        target = SCROLL_GROUP_OFFSET + start
        saved[target : target + RECORD_SIZE] = record
    return bytes(saved)


def dispatch_frame(function_address: int) -> dict:
    """The preserved-context frame `verify_dispatch` accepts."""

    before_rsp = 0x10000008
    after_rsp = before_rsp - 0x48
    left = before_rsp - 16
    result = (left - 0x38) & ((1 << 64) - 1)
    eflags = (
        int(left < 0x38)
        | ((bin(result & 255).count("1") % 2 == 0) << 2)
        | (int((left ^ 0x38 ^ result) & 16 != 0) << 4)
        | (int(result == 0) << 6)
        | (((result >> 63) & 1) << 7)
        | (int((left ^ 0x38) & (left ^ result) & (1 << 63) != 0) << 11)
    )
    registers = {
        name: 0x414100000000 + len(name)
        for name in (
            "RAX",
            "RBX",
            "RCX",
            "RDX",
            "RSI",
            "RDI",
            "RBP",
            "R8",
            "R9",
            "R10",
            "R11",
            "R12",
            "R13",
            "R14",
            "R15",
        )
    }
    before = dict(registers, RSP=before_rsp, RIP=function_address, EFLAGS=0x202)
    after = dict(
        registers,
        RSP=after_rsp,
        RIP=function_address + 7,
        EFLAGS=(0x202 & ~0x8D5) | eflags,
    )
    return {"before": before, "after": after}


class _StubCrypto:
    """The save side decrypts in place; the stub copies the bytes."""

    def decrypt(self, source: Path, destination: Path) -> Path:
        destination.write_bytes(source.read_bytes())
        return destination


class _ScenarioAdapter:
    """The injected native executor both applications are compared through."""

    def __init__(self, container: bytes, serial_counter: int, acquisition_order: int, fault: str):
        self.container = bytearray(container)
        self.serial_counter = serial_counter
        self.acquisition_order = acquisition_order
        self.fault = fault
        self.receipts: dict[str, dict] = {}
        self.submissions: list[str] = []
        self.current_creation = str(CREATION_LATER if fault == "pid_reuse" else CREATION)

    def _inventory(self) -> dict:
        entries = []
        for slot in range(CAPACITY):
            record = bytes(self.container[slot * RECORD_SIZE : (slot + 1) * RECORD_SIZE])
            if record[:2] == b"\0\0":
                continue
            entries.append(
                {
                    "slot_index": slot,
                    "record_hex": record.hex(),
                    "serial": str(struct.unpack_from("<Q", record, 0x28)[0]),
                    "seed": struct.unpack_from("<I", record, 0x20)[0],
                }
            )
        return {
            "pid": FIXTURE_PID,
            "process_creation_time": str(CREATION),
            "capacity": CAPACITY,
            "entries": entries,
            "duplicate_scroll_serials": [],
            "serial_counter": str(self.serial_counter),
            "acquisition_order_counter": self.acquisition_order,
            "container_sha256": hashlib.sha256(self.container).hexdigest(),
        }

    def _index(self) -> dict:
        entries = [
            {"serial": entry["serial"], "slot": entry["slot_index"]}
            for entry in self._inventory()["entries"]
        ]
        return {
            "node_count": len(entries),
            "entries": entries,
            "pid": FIXTURE_PID,
            "process_creation_time": str(CREATION),
        }

    def inspect(self):
        slots = [
            slot
            for slot in range(CAPACITY)
            if self.container[slot * RECORD_SIZE : slot * RECORD_SIZE + 2] == b"\0\0"
        ]
        if not slots:
            raise ValueError("Scroll inventory is full")
        plan = {
            "pid": FIXTURE_PID,
            "profile_id": PROFILE_ID,
            "manager": MANAGER,
            "data": DATA,
            "process_creation_time": str(CREATION),
            "serial": self.serial_counter,
            "slot": slots[0],
            "scheduler_owner": BASE + 0x9000,
            "function_address": BASE + LAYOUT.insertion_rva,
            "container_hex": bytes(self.container).hex(),
            "insertion_code_hex": "40" * 8,
            "builder_code_hex": assembly_descriptor(fixture_record()).hex(),
        }
        return plan, self._inventory(), self._index()

    def preview(self, plan, installation_record):
        source = bytearray(installation_record)
        source[0x28:0x30] = b"\xff" * 8
        verify_assembly_preview(installation_record, bytes(source))
        receipt = dispatch_frame(plan["function_address"])
        receipt.update(
            phase="completed",
            redirect_count=1,
            released=True,
            active=False,
            breakpoints=[],
            pid=plan["pid"],
            source_hex=bytes(source).hex(),
        )
        verify_dispatch(receipt)
        return receipt

    def insert(self, plan):
        operation_id = plan["operation_id"]
        self.submissions.append(operation_id)
        if self.fault == "absent":
            raise RuntimeError("submission rejected before dispatch")
        if self.fault == "idle_miss":
            receipt = dispatch_frame(plan["function_address"])
            receipt.update(
                phase="released",
                redirect_count=0,
                released=True,
                active=False,
                breakpoints=[],
                operation_id=operation_id,
                pid=plan["pid"],
                error="No accepted idle dispatch before timeout",
            )
            self.receipts[operation_id] = receipt
            return receipt
        source = bytearray(bytes.fromhex(plan["expected_record_hex"]))
        struct.pack_into("<Q", source, 0x28, plan["serial"])
        destination = bytearray(source)
        struct.pack_into(
            "<I",
            destination,
            0x18,
            struct.unpack_from("<I", destination, 0x18)[0] | 0x04000080,
        )
        struct.pack_into("<I", destination, 0x1C, self.acquisition_order)
        start = plan["slot"] * RECORD_SIZE
        self.container[start : start + RECORD_SIZE] = destination
        self.acquisition_order += 1
        self.serial_counter = plan["serial"] + 1
        if self.fault == "readback_changed":
            untouched = 399 * RECORD_SIZE
            self.container[untouched : untouched + 2] = b"\x01\x02"
        receipt = dispatch_frame(plan["function_address"])
        receipt.update(
            phase="completed",
            redirect_count=1,
            released=True,
            active=False,
            breakpoints=[],
            mode="single_native_insertion",
            operation_id=operation_id,
            status=3,
            slot=plan["slot"],
            pid=plan["pid"],
            source_hex=bytes(source).hex(),
            destination_hex=bytes(destination).hex(),
            remainder_hex=(b"\0" * RECORD_SIZE).hex(),
            process_creation_time=str(CREATION),
        )
        self.receipts[operation_id] = receipt
        if self.fault == "reply_lost":
            raise RuntimeError("native reply lost; query the receipt")
        return receipt

    def readback(self):
        return self._inventory(), self._index()

    def recover(self, operation_id, pid, process_creation_time=None):
        self.require_process_instance(pid, process_creation_time)
        return self.receipts[operation_id]

    def require_process_instance(self, pid, creation_time):
        if creation_time != self.current_creation:
            raise RuntimeError(
                "PROCESS_INSTANCE_CHANGED: do not verify an old receipt against a new game"
            )
        return None

    def submission_absent(self, operation_id):
        return self.fault == "absent"

    def safe_to_shutdown(self):
        return True


def python_scenario(spec: dict) -> list[str]:
    """The same step script the Rust probe runs, over the shipped classes."""

    root = Path(tempfile.mkdtemp(prefix="nioh3-live-parity-"))
    lines: list[str] = []
    try:
        save_directory = root / "76561198000000000" / "SAVEDATA00"
        save_directory.mkdir(parents=True)
        save = save_directory / "SAVEDATA.BIN"
        save.write_bytes(bytes.fromhex(spec["save_hex"]))
        adapter = _ScenarioAdapter(
            bytes.fromhex(spec["container_hex"]),
            spec["serial_counter"],
            spec.get("acquisition_order_counter", 0),
            spec.get("fault", "none"),
        )
        application = LiveAddApplication(
            root, spec["context_digest"], adapter=adapter, crypto=_StubCrypto()
        )
        batch = LiveAddBatch(application)
        operation_id = plan_digest = batch_id = batch_digest = ""
        for index, step in enumerate(spec["steps"]):
            if step == "prepare":
                try:
                    prepared = application.prepare(spec["candidates"][0], save)
                except Exception as error:  # noqa: BLE001 - the shipped text is the contract
                    lines.append(f"prepared\terror\t{error}")
                    continue
                operation_id = prepared["operation_id"]
                plan_digest = prepared["plan_digest"]
                lines.append(
                    "prepared\t{}\t{}\t{}\t{}".format(
                        prepared["state"],
                        prepared["seed"],
                        prepared["rarity"],
                        prepared["count_before"],
                    )
                )
            elif step in ("execute", "recover", "status", "cancel"):
                try:
                    if step == "execute":
                        value = application.execute(operation_id, plan_digest)
                    else:
                        value = getattr(application, step)(operation_id)
                except Exception as error:  # noqa: BLE001
                    lines.append(f"step\t{index}\terror\t{error}")
                    continue
                receipt = value.get("receipt") or {}
                lines.append(
                    f"step\t{index}\t{value['state']}\t{receipt.get('error') or '-'}"
                )
            elif step == "batch_prepare":
                try:
                    value = batch.prepare(spec["candidates"], save)
                except Exception as error:  # noqa: BLE001
                    lines.append(f"batch\terror\t{error}\t0")
                    continue
                batch_id = value["batch_id"]
                batch_digest = value["plan_digest"]
                lines.append(f"batch\t{value['state']}\t{value['count']}\t0")
            elif step == "batch_execute":
                limit = spec.get("cancel_after")
                seen = 0

                def cancelled() -> bool:
                    nonlocal seen
                    seen += 1
                    return limit is not None and seen > limit

                try:
                    value = batch.execute(batch_id, batch_digest, cancelled=cancelled)
                except Exception as error:  # noqa: BLE001
                    lines.append(f"batch\terror\t{error}\t0")
                    continue
                lines.append(f"batch\t{value['state']}\t{value['verified_count']}\t0")
            elif step == "batch_status":
                try:
                    value = batch.status(batch_id)
                except Exception as error:  # noqa: BLE001
                    lines.append(f"batch\terror\t{error}\t0")
                    continue
                lines.append(f"batch\t{value['state']}\t0\t{len(value['children'])}")
            elif step == "batch_cancel":
                try:
                    value = batch.cancel(batch_id)
                except Exception as error:  # noqa: BLE001
                    lines.append(f"batch\terror\t{error}\t0")
                    continue
                lines.append(f"batch\t{value['state']}\t0\t0")
            else:
                raise AssertionError(f"unknown scenario step {step}")
        return lines
    finally:
        shutil.rmtree(root, ignore_errors=True)


def candidate_payload(seed: int, rarity: int = 4, record: bytes | None = None) -> dict:
    """One transferred candidate in the broker-owned shape."""

    from nioh3_scroll_editor.core_services import candidate_identity
    from nioh3_scroll_editor.models import CandidateRecordStage, ScrollCandidate

    raw = record if record is not None else fixture_record(seed=seed, rarity=rarity)
    candidate = ScrollCandidate(
        seed=seed,
        record=raw,
        effects=(),
        rarity=rarity,
        playthrough=2,
        record_stage=CandidateRecordStage.FINAL_RECORD,
    )
    return {
        "candidate_id": candidate_identity(candidate, CONTEXT_DIGEST),
        "context_digest": CONTEXT_DIGEST,
        "level": 1,
        "seed": seed,
        "playthrough": 2,
        "rarity": rarity,
        "record_stage": "final_record",
        "record_hex": raw.hex(),
        "installation_record_hex": None,
        "effects": [],
    }


class RuntimeLiveAddParityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.binary = build_probe()

    def probe(self, *arguments: str) -> list[str]:
        completed = subprocess.run(
            [str(type(self).binary), *arguments],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
            timeout=600,
            check=False,
        )
        if completed.returncode != 0:
            raise AssertionError(
                f"runtime_read_probe {arguments[:1]} failed: "
                + (completed.stdout + completed.stderr).strip()
            )
        return [line for line in completed.stdout.splitlines() if line]

    def probe_scenario(self, mode: str, spec: dict) -> list[str]:
        """A synthetic 400-record container does not fit a Windows command line."""

        handle, name = tempfile.mkstemp(prefix="nioh3-live-spec-", suffix=".json")
        os.close(handle)
        path = Path(name)
        try:
            path.write_text(json.dumps(spec), encoding="utf-8")
            return self.probe(mode, str(path))
        finally:
            path.unlink(missing_ok=True)

    def test_assembly_descriptor_matches_the_shipped_builder(self) -> None:
        cases = [
            fixture_record(),
            fixture_record(seed=1, rarity=3),
            fixture_record(seed=0xFFFFFFFF, rarity=5, record_type=0x516D),
            fixture_record(record_type=0xE604),
            fixture_record(seed=0),
            fixture_record(rarity=2),
            bytes(RECORD_SIZE).hex(),
        ]
        for index, record in enumerate(cases):
            with self.subTest(case=index):
                rows = self.probe(
                    "--descriptor",
                    json.dumps(
                        {
                            "record_hex": record
                            if isinstance(record, str)
                            else bytes(record).hex(),
                            "allocate_serial": index % 2 == 0,
                        }
                    ),
                )
                self.assertEqual(
                    rows,
                    self.shipped_descriptor(
                        record if isinstance(record, str) else bytes(record).hex(),
                        index % 2 == 0,
                    ),
                )

    @staticmethod
    def shipped_descriptor(record: str, allocate: bool) -> list[str]:
        raw = bytes.fromhex(record)
        rows = []
        try:
            rows.append("descriptor\t" + assembly_descriptor(raw, allocate_serial=allocate).hex())
        except Exception as error:  # noqa: BLE001 - the shipped text is the contract
            rows.append(f"error\tCANDIDATE_REJECTED\t{error}")
        try:
            rows.append("assembly\t" + new_assembly_record(raw).hex())
        except Exception as error:  # noqa: BLE001
            rows.append(f"error\tCANDIDATE_REJECTED\t{error}")
        return rows

    def test_inventory_capture_matches_the_shipped_reader(self) -> None:
        container = container_bytes(
            [(0, 0x1122334455667788, 0x0BAD), (7, 0x99, 0x0BEE), (399, 0x5, 1)],
            acquisition_order=11,
        )
        serial_counter = 0x1122334455667789
        request = json.dumps(
            {
                "container_hex": container.hex(),
                "serial_counter": serial_counter,
                "acquisition_order_counter": 11,
            }
        )
        self.assertEqual(
            self.probe_scenario("--inventory", json.loads(request)),
            self.shipped_inventory(container, serial_counter, 11),
        )

        for corruption in ("signature", "capacity", "owner"):
            with self.subTest(corruption=corruption):
                spec = {
                    "container_hex": container.hex(),
                    "serial_counter": serial_counter,
                    "acquisition_order_counter": 11,
                    "corrupt": corruption,
                }
                self.assertEqual(
                    self.probe_scenario("--inventory", spec),
                    self.shipped_inventory(container, serial_counter, 11, corruption),
                )

    @staticmethod
    def shipped_inventory(
        container: bytes, serial_counter: int, acquisition_order: int, corrupt: str = "none"
    ) -> list[str]:
        memory: dict[int, bytes] = {
            BASE + LAYOUT.insertion_rva: bytes.fromhex("40555356574154415541564157488DAC"),
            BASE + LAYOUT.manager_pointer_rva: struct.pack("<Q", MANAGER),
            MANAGER: struct.pack("<Q", DATA),
            DATA: struct.pack("<Q", acquisition_order) + struct.pack("<Q", serial_counter),
            CONTAINER: container,
            CONTAINER + LAYOUT.capacity_offset: struct.pack("<Q", 400),
        }
        if corrupt == "signature":
            memory[BASE + LAYOUT.insertion_rva] = b"\0" * 16
        elif corrupt == "capacity":
            memory[CONTAINER + LAYOUT.capacity_offset] = struct.pack("<Q", 399)
        elif corrupt == "owner":
            memory[BASE + LAYOUT.manager_pointer_rva] = b"\0" * 8

        class _FakeReader:
            pid = FIXTURE_PID
            module_base = BASE

            def __enter__(self):
                return self

            def __exit__(self, *exc):
                return False

            def read(self, address, size):
                data = memory.get(address)
                if data is None or len(data) < size:
                    raise RuntimeError(f"short read at {address:#x}")
                return bytes(data[:size])

            def u64(self, address):
                return struct.unpack_from("<Q", self.read(address, 8))[0]

            def creation_time(self):
                return str(CREATION)

        def identity():
            return (FIXTURE_PID, SimpleNamespace(display_version="PC v2.01"), "Nioh3.exe")

        with mock.patch.object(live_inventory, "ProcessReader", _FakeReader), mock.patch.object(
            live_inventory, "running_game_identity", identity
        ):
            try:
                value = live_inventory.capture_inventory()
            except Exception as error:  # noqa: BLE001
                return [f"error\tINVENTORY_INVALID\t{error}"]
        rows = [
            f"pid\t{value['pid']}",
            f"creation\t{value['process_creation_time']}",
            f"capacity\t{value['capacity']}",
        ]
        for entry in value["entries"]:
            rows.append("entry\t{slot_index}\t{serial}\t{seed}\t{record_hex}".format(**entry))
        rows.extend(f"duplicate\t{serial}" for serial in value["duplicate_scroll_serials"])
        rows.append(f"serial_counter\t{value['serial_counter']}")
        rows.append(f"acquisition_order_counter\t{value['acquisition_order_counter']}")
        rows.append(f"container_sha256\t{value['container_sha256']}")
        return rows

    def test_candidate_transfer_gates_match_the_shipped_importer(self) -> None:
        cases = [
            candidate_payload(0x0BAD),
            {**candidate_payload(0x0BAD), "context_digest": "another"},
            {**candidate_payload(0x0BAD), "candidate_id": "00"},
            {
                **candidate_payload(0x0BAD),
                "record_stage": "native_stage_one",
                "candidate_id": "00",
            },
        ]
        for index, payload in enumerate(cases):
            with self.subTest(case=index):
                container = container_bytes([(0, 0x1000, 0x11)], acquisition_order=0)
                rows = self.probe_scenario(
                    "--live-add-scenario",
                    {
                        "container_hex": container.hex(),
                        "serial_counter": 0x1001,
                        "acquisition_order_counter": 0,
                        "save_hex": save_bytes(container).hex(),
                        "context_digest": CONTEXT_DIGEST,
                        "fault": "none",
                        "candidates": [payload],
                        "steps": ["prepare"],
                    },
                )
                expected = self.shipped_candidate_gate(payload)
                if not expected:
                    # A valid transfer reaches the prepared snapshot the probe
                    # publishes: one occupied record, so `count_before` is 1.
                    expected = [
                        "prepared\tprepared\t{}\t{}\t1".format(
                            payload["seed"], payload["rarity"]
                        )
                    ]
                self.assertEqual(
                    [row for row in rows if row.startswith(("prepared", "step", "error"))],
                    expected,
                )

    @staticmethod
    def shipped_candidate_gate(payload: dict) -> list[str]:
        try:
            value = import_candidate(payload, CONTEXT_DIGEST)
            require_search_candidate_ready(value)
            from nioh3_scroll_editor.core_services import OperationCommand, OperationPolicy
            from nioh3_scroll_editor.models import CandidateRecordStage

            OperationPolicy().require(OperationCommand.INSTALL_GENERATED, candidate=value)
            if value.record_stage is not CandidateRecordStage.FINAL_RECORD:
                raise ValueError("Materialize and finalize the candidate before live addition")
            new_assembly_record(value.installation_record or value.record)
        except Exception as error:  # noqa: BLE001
            return [f"prepared\terror\t{error}"]
        return []

    def test_live_add_scenarios_match_the_shipped_application(self) -> None:
        for fault in ("none", "reply_lost", "absent", "idle_miss", "readback_changed", "pid_reuse"):
            with self.subTest(fault=fault):
                container = container_bytes([(0, 0x1000, 0x11)], acquisition_order=5)
                spec = {
                    "container_hex": container.hex(),
                    "serial_counter": 0x1001,
                    "acquisition_order_counter": 5,
                    "save_hex": save_bytes(container).hex(),
                    "context_digest": CONTEXT_DIGEST,
                    "fault": fault,
                    "candidates": [candidate_payload(0x0BAD)],
                    "steps": ["prepare", "execute", "status", "execute", "recover"],
                }
                self.assertEqual(
                    self.probe_scenario("--live-add-scenario", spec),
                    python_scenario(spec),
                    fault,
                )

    def test_batch_scenarios_match_the_shipped_application(self) -> None:
        cases = [
            {"cancel_after": None, "steps": ["batch_prepare", "batch_execute", "batch_status"]},
            {"cancel_after": 1, "steps": ["batch_prepare", "batch_execute", "batch_status"]},
            {"cancel_after": 0, "steps": ["batch_prepare", "batch_execute", "batch_status"]},
            {"cancel_after": None, "steps": ["batch_prepare", "batch_cancel", "batch_status"]},
            {
                "cancel_after": None,
                "steps": ["batch_prepare", "batch_execute", "batch_execute"],
                "state_only": True,
            },
        ]
        for index, case in enumerate(cases):
            with self.subTest(case=index):
                container = container_bytes([(0, 0x2000, 0x21)], acquisition_order=3)
                spec = {
                    "container_hex": container.hex(),
                    "serial_counter": 0x2001,
                    "acquisition_order_counter": 3,
                    "save_hex": save_bytes(container).hex(),
                    "context_digest": CONTEXT_DIGEST,
                    "fault": "none",
                    "candidates": [
                        candidate_payload(0x0D00),
                        candidate_payload(0x0D01),
                        candidate_payload(0x0D02),
                    ],
                    "cancel_after": case["cancel_after"],
                    "steps": case["steps"],
                }
                rust = self.probe_scenario("--live-add-scenario", spec)
                python = python_scenario(spec)
                if case.get("state_only"):
                    # A second batch execution is refused by the exclusive claim
                    # on both sides; only the OS-level failure text differs.
                    self.assertEqual(
                        [row.split("\t")[:2] for row in rust],
                        [row.split("\t")[:2] for row in python],
                        str(case),
                    )
                else:
                    self.assertEqual(rust, python, str(case))

    def test_the_new_modules_keep_the_scoped_rights_contract(self) -> None:
        for name in MUTATION_SOURCES:
            source = (CRATE / "src" / name).read_text(encoding="utf-8")
            text = strip_string_literals(strip_comments(source))
            for capability in FORBIDDEN_EVERYWHERE:
                with self.subTest(source=name, capability=capability):
                    self.assertNotIn(capability, text, f"{capability} in {name}")

        native_source = (ROOT / "nioh3_scroll_editor" / "native.py").read_text(encoding="utf-8")
        self.assertIn("CreateRemoteThread", native_source)
        self.assertIn("PROCESS_ACCESS = 0x0002 | 0x0008 | 0x0010 | 0x0020 | 0x0400", native_source)


def strip_comments(text: str) -> str:
    kept = []
    for line in text.splitlines():
        if line.strip().startswith("//"):
            continue
        kept.append(line.split("//", 1)[0])
    return "\n".join(kept)


def strip_string_literals(text: str) -> str:
    out = []
    in_string = False
    escaped = False
    for character in text:
        if in_string:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                in_string = False
            continue
        if character == '"':
            in_string = True
            continue
        out.append(character)
    return "".join(out)


if __name__ == "__main__":
    unittest.main()
