"""Read an existing PC202 observation's complete address set twice and retain a matched pair."""

import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
from pathlib import Path
import struct
import time


def capture(observation: Path, output: Path, timeout: float):
    snapshot = json.loads((observation / "snapshot.json").read_bytes())
    reference = json.loads((observation / "reference-target.json").read_bytes())
    if (
        reference["process"] != snapshot["process"]
        or reference["actor"] != snapshot["roots"]["player"]
    ):
        raise ValueError("Observation and target identities differ")
    if timeout <= 0 or output.exists():
        raise ValueError("Use a positive timeout and a fresh output directory")
    starts = set()
    with (observation / "ordered-reads.jsonl").open(encoding="utf8") as stream:
        for line in stream:
            row = json.loads(line)
            starts.add(
                (row["address"], row["address"] + len(bytes.fromhex(row["bytes"])))
            )
    spans = []
    for start, end in sorted(starts):
        if spans and start <= spans[-1][1]:
            spans[-1][1] = max(end, spans[-1][1])
        else:
            spans.append([start, end])
    selected_field = snapshot["roots"]["style_entity"] + 0xB98
    selected_region = next(
        i
        for i, (a, b) in enumerate(spans)
        if a <= selected_field and selected_field + 8 <= b
    )
    pos = selected_field - spans[selected_region][0]
    proc = snapshot["process"]
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.CloseHandle.argtypes = (wintypes.HANDLE,)
    kernel.GetProcessTimes.argtypes = (
        wintypes.HANDLE,
        *(ctypes.POINTER(wintypes.FILETIME),) * 4,
    )
    kernel.ReadProcessMemory.argtypes = (
        wintypes.HANDLE,
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_size_t,
        ctypes.POINTER(ctypes.c_size_t),
    )
    handle = kernel.OpenProcess(0x1000 | 0x10, False, proc["pid"])
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())

    def identity():
        times = [wintypes.FILETIME() for _ in range(4)]
        if not kernel.GetProcessTimes(handle, *(ctypes.byref(t) for t in times)):
            raise ctypes.WinError(ctypes.get_last_error())
        stamp = times[0].dwLowDateTime | times[0].dwHighDateTime << 32
        if stamp != proc["creation_filetime"]:
            raise ValueError("Process lifetime changed")

    try:
        identity()
        blocks = [(a, ctypes.create_string_buffer(b - a)) for a, b in spans]
        size = ctypes.c_size_t()

        def sweep():
            fields = []
            for a, buf in blocks:
                n = ctypes.sizeof(buf)
                if (
                    not kernel.ReadProcessMemory(handle, a, buf, n, ctypes.byref(size))
                    or size.value != n
                ):
                    raise OSError(f"Incomplete read at {a:#x}")
                fields.append(buf.raw)
            return fields

        deadline = time.monotonic() + timeout
        attempts = 0
        while time.monotonic() < deadline:
            attempts += 1
            start = time.perf_counter()
            first = sweep()
            second = sweep()
            elapsed = (time.perf_counter() - start) * 1000
            if (
                first != second
                or struct.unpack_from("<Q", first[selected_region], pos)[0]
                != reference["opponent"]["actor"]
            ):
                time.sleep(0.02)
                continue
            identity()
            result = dict(
                pid=proc["pid"],
                creation_filetime=proc["creation_filetime"],
                module_base=snapshot["module_base"],
                regions=[
                    dict(address=a, bytes=b.hex())
                    for (a, _), b in zip(spans, first, strict=True)
                ],
            )
            raw = json.dumps(result, separators=(",", ":")).encode()
            output.mkdir()
            (output / "ordinary-read-memory.json").write_bytes(raw)
            (output / "receipt.json").write_text(
                json.dumps(
                    dict(
                        scope="Two byte-identical full read sweeps; downstream role replay required",
                        process=proc,
                        selected_target=reference["opponent"]["actor"],
                        attempts=attempts,
                        elapsed_ms=elapsed,
                        spans=len(spans),
                        bytes=sum(map(len, first)),
                        sha256=hashlib.sha256(raw).hexdigest(),
                        game_writes=0,
                    ),
                    indent=2,
                ),
                encoding="utf8",
            )
            return
        raise TimeoutError(
            "No identical pair with the selected opponent during the acquisition"
        )
    finally:
        kernel.CloseHandle(handle)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("observation", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--timeout", type=float, default=15)
    capture(**vars(parser.parse_args()))
