# /// script
# requires-python = ">=3.11"
# dependencies = ["pyelftools"]
# ///
"""Print FLASH/RAM usage of a firmware ELF against the regions in memory.x."""

import re
import sys
from pathlib import Path

from elftools.elf.constants import SH_FLAGS
from elftools.elf.elffile import ELFFile

REGION_RE = re.compile(
    r"^\s*(\w+)\s*:\s*ORIGIN\s*=\s*(\w+)\s*,\s*LENGTH\s*=\s*(\w+)", re.MULTILINE
)
SUFFIX = {"K": 1024, "M": 1024 * 1024}


def parse_num(s: str) -> int:
    mult = SUFFIX.get(s[-1].upper(), 1)
    return int(s[:-1] if mult > 1 else s, 0) * mult


def parse_memory_x(path: Path) -> dict[str, tuple[int, int]]:
    text = re.sub(r"/\*.*?\*/", "", path.read_text(), flags=re.DOTALL)
    regions = {
        name: (parse_num(org), parse_num(length))
        for name, org, length in REGION_RE.findall(text)
    }
    if "SRAM" in regions:
        regions.setdefault("RAM", regions["SRAM"])
    return regions


def main() -> int:
    elf_path, memory_x = Path(sys.argv[1]), Path(sys.argv[2])
    regions = parse_memory_x(memory_x)
    missing = {"FLASH", "RAM"} - regions.keys()
    if missing:
        print(f"{memory_x}: region(s) not found: {', '.join(sorted(missing))}", file=sys.stderr)
        return 1

    (flash_org, flash_len), (ram_org, ram_len) = regions["FLASH"], regions["RAM"]
    used = {"FLASH": 0, "RAM": 0}
    with elf_path.open("rb") as f:
        for sec in ELFFile(f).iter_sections():
            size, addr = sec["sh_size"], sec["sh_addr"]
            if not sec["sh_flags"] & SH_FLAGS.SHF_ALLOC or size == 0:
                continue
            if flash_org <= addr < flash_org + flash_len:
                used["FLASH"] += size
            elif ram_org <= addr < ram_org + ram_len:
                used["RAM"] += size
                # initialized RAM (.data) is copied from its load image in FLASH
                if sec["sh_type"] != "SHT_NOBITS":
                    used["FLASH"] += size

    print()
    for name, total in (("FLASH", flash_len), ("RAM", ram_len)):
        n = used[name]
        print(f"{name:<6} {n / 1024:8.2f} KiB / {total / 1024:7.2f} KiB  {100 * n / total:6.2f}%")
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
