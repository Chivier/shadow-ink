#!/usr/bin/env python3
"""Collect available dependency licenses and embedded native copyright headers."""
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--locked"], cwd=root))
sections = ["Third-party notices for shadow-ink\n\nDependency sources, licenses and native attribution headers follow.\n"]
for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
    if package["source"] is None:
        continue
    directory = Path(package["manifest_path"]).parent
    sections.append(f"\n{'='*72}\n{package['name']} {package['version']}\nLicense: {package['license']}\nAuthors: {', '.join(package['authors'])}\nSource: {package['repository'] or package['source']}\n")
    seen = set()
    for path in sorted(directory.rglob("*")):
        if not path.is_file():
            continue
        name = path.name.upper()
        text = None
        if name.startswith(("LICENSE", "COPYING", "NOTICE", "COPYRIGHT", "UNLICENSE")) and path.stat().st_size < 200000:
            text = path.read_text(errors="replace")
        elif path.suffix in {".c", ".cpp", ".cc", ".h", ".hpp", ".rs"}:
            with path.open(errors="replace") as stream:
                first = stream.read(12000).lstrip()
            if first.startswith("/*") and "*/" in first:
                candidate = first.split("*/", 1)[0] + "*/"
            elif first.startswith("//"):
                comment = []
                for line in first.splitlines():
                    if not line.startswith("//"):
                        break
                    comment.append(line)
                candidate = "\n".join(comment)
            else:
                candidate = ""
            if "copyright" in candidate.lower() or "spdx-license" in candidate.lower():
                text = candidate
        if text and text not in seen:
            seen.add(text)
            sections.append(f"\n--- {path.relative_to(directory)} ---\n{text}\n")
for path in sorted((root / "docs/licenses").iterdir()):
    if path.is_file():
        sections.append(f"\n--- {path.name} ---\n" + path.read_text())
Path(sys.argv[1]).write_text("".join(sections))
