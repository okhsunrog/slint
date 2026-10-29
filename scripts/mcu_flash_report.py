#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
"""Report load addresses and linker extent from an LLD map for an MCU build."""
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("maps", type=Path, nargs="+")
parser.add_argument("--origin", type=lambda s: int(s, 0), default=0x4000)
parser.add_argument("--length", type=lambda s: int(s, 0), default=0x6FFFE)
parser.add_argument("--ram-origin", type=lambda s: int(s, 0), default=0x1FFF8000)
args = parser.parse_args()
reports = []
for path in args.maps:
    sections = {}
    output_column = path.read_text().splitlines()[0].index("Out")
    for line in path.read_text().splitlines():
        fields = line.split()
        if len(fields) != 5 or not fields[4].startswith(".") or line.index(fields[4]) != output_column:
            continue
        vma, lma, size = (int(value, 16) for value in fields[:3])
        sections[fields[4]] = {"vma": vma, "lma": lma, "size": size}
    flash = [s for s in sections.values() if args.origin <= s["lma"] < args.ram_origin]
    if not flash:
        raise ValueError(f"No flash sections in {path}")
    # LLD checks the aligned address of empty output sections too.
    extent = max(s["lma"] + s["size"] for s in flash) - args.origin
    reports.append({"map": path.name, "flash_extent": extent,
                    "remaining": args.length - extent,
                    "flash_section_bytes": sum(s["size"] for s in flash),
                    "sections": sections})
print(json.dumps(reports, indent=2))
