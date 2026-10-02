#!/usr/bin/env python3
"""Generate tiny project-owned PNG fixtures for the non-Minecraft sample packages."""

from pathlib import Path
import struct
import zlib


def png(path: Path, pixel):
    width = height = 16
    rows = []
    for y in range(height):
        row = bytearray([0])
        for x in range(width):
            row.extend(pixel(x, y))
        rows.append(row)
    raw = b"".join(rows)

    def chunk(kind, payload):
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload))

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


root = Path(__file__).resolve().parents[1] / "crates" / "sandbox-test" / "test-packages"
png(root / "base/assets/sandbox_test/textures/block/foundation.png", lambda x, y: (42 + (x + y) % 2 * 10, 57, 77, 255))
png(root / "base/assets/sandbox_test/textures/block/crystal.png", lambda x, y: (20, 150 + (x ^ y) % 4 * 20, 220, 255))
png(root / "base/assets/sandbox_test/textures/block/pulse.png", lambda x, y: (240, 40 + (x + y) % 3 * 20, 160, 255))
png(root / "mod/assets/sandbox_test/textures/block/crystal.png", lambda x, y: (80, 230, 130 + (x ^ y) % 3 * 30, 255))
png(root / "mod/assets/sandbox_mod/textures/block/reactor.png", lambda x, y: (250, 150 + (x + y) % 2 * 60, 30, 255))
