"""Guards for the Windows icon that Tauri turns into the app window icon.

Tauri's build-time codegen embeds ``default_window_icon`` from the *first* ICO
directory entry (``tauri-codegen`` ``image.rs``: ``let entry =
&icon_dir.entries()[0];``) and tao passes that single bitmap to
``WM_SETICON(ICON_SMALL)``. A small first frame is therefore upscaled by the
taskbar, which produced the blurry window icon players reported. These two
guards keep the configured Windows icon high-resolution and structurally valid.
"""
from __future__ import annotations

import json
import struct
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TAURI_CONF = ROOT / "apps" / "tauri" / "src-tauri" / "tauri.conf.json"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
REQUIRED_NATIVE_SIZES = frozenset({16, 32, 48, 256})
HEADER = struct.Struct("<HHH")
ENTRY = struct.Struct("<BBBBHHII")


def configured_windows_icon() -> Path:
    """Return the .ico path the Tauri build actually consumes."""

    config = json.loads(TAURI_CONF.read_text(encoding="utf-8"))
    for entry in config["bundle"]["icon"]:
        if entry.lower().endswith(".ico"):
            return (TAURI_CONF.parent / entry).resolve()
    raise AssertionError("tauri bundle config declares no Windows .ico icon")


def read_frames(path: Path) -> tuple[bytes, list[dict]]:
    data = path.read_bytes()
    reserved, kind, count = HEADER.unpack_from(data, 0)
    if (reserved, kind) != (0, 1):
        raise AssertionError(f"{path.name} is not a valid ICO container")
    frames = []
    for index in range(count):
        width, height, _colors, _reserved, _planes, bpp, size, offset = ENTRY.unpack_from(
            data, HEADER.size + index * ENTRY.size
        )
        frames.append(
            {
                "width": width or 256,
                "height": height or 256,
                "bpp": bpp,
                "size": size,
                "offset": offset,
                "blob": data[offset : offset + size],
            }
        )
    return data, frames


def payload_dimensions(blob: bytes) -> tuple[int, int]:
    """Read the pixel size a frame actually carries."""

    if blob[:8] == PNG_SIGNATURE:
        return struct.unpack_from(">II", blob, 16)
    header_size = struct.unpack_from("<i", blob, 0)[0]
    if header_size < 40:
        raise AssertionError("icon frame carries neither a PNG nor a DIB header")
    width = struct.unpack_from("<i", blob, 4)[0]
    height = struct.unpack_from("<i", blob, 8)[0]
    # An ICO DIB height covers the XOR image plus its AND mask.
    return width, abs(height) // 2


class IconResourceTests(unittest.TestCase):
    def test_configured_windows_icon_starts_with_its_largest_frame(self) -> None:
        _data, frames = read_frames(configured_windows_icon())
        first = frames[0]
        largest = max(frame["width"] * frame["height"] for frame in frames)
        self.assertEqual(
            first["width"] * first["height"],
            largest,
            "the first ICO frame becomes the Tauri window icon, so it must be the "
            "largest frame instead of a small bitmap the taskbar has to upscale",
        )
        self.assertGreaterEqual(
            min(first["width"], first["height"]),
            256,
            "the window icon frame must stay high resolution",
        )

    def test_configured_windows_icon_frames_are_valid_native_layers(self) -> None:
        path = configured_windows_icon()
        data, frames = read_frames(path)
        self.assertTrue(
            REQUIRED_NATIVE_SIZES <= {frame["width"] for frame in frames},
            f"icon must keep the native frame sizes {sorted(REQUIRED_NATIVE_SIZES)}",
        )
        for frame in frames:
            with self.subTest(size=f"{frame['width']}x{frame['height']}"):
                self.assertGreater(frame["size"], 0)
                self.assertLessEqual(
                    frame["offset"] + frame["size"],
                    len(data),
                    "frame payload runs past the end of the ICO file",
                )
                self.assertEqual(len(frame["blob"]), frame["size"])
                self.assertEqual(
                    payload_dimensions(frame["blob"]),
                    (frame["width"], frame["height"]),
                    "directory entry dimensions disagree with the frame payload",
                )


if __name__ == "__main__":
    unittest.main()
