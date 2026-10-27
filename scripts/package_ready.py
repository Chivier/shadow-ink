#!/usr/bin/env python3
"""Reject missing, partial or stale release evidence before packaging."""
import hashlib
import json
from pathlib import Path
import sys
from source_fingerprint import source_fingerprint

root = Path(__file__).resolve().parents[1]
report = root / "acceptance-output"
checks = json.loads((report / "checks.json").read_text())
assert checks["all_passed"] and len(checks["checks"]) == 8 and all(c["exit_code"] == 0 for c in checks["checks"]), "Run scripts/verify.py to completion first"
assert checks["source_sha256"] == source_fingerprint(root), "Source changed after verification; run scripts/verify.py again"
for name in ["tmux", "video"]:
    result = json.loads((report / f"{name}.json").read_text())
    assert result["final_report"]["complete"] and result["final_report"]["status"]["finalized"]
if len(sys.argv) > 1 and sys.argv[1] == "static":
    artifacts = json.loads((report / "artifacts.json").read_text())
    assert artifacts["all_passed"]
    for arch, expected in artifacts["artifacts"].items():
        binary = root / f"target/{arch}-unknown-linux-musl/release/shadow-ink"
        assert hashlib.sha256(binary.read_bytes()).hexdigest() == expected["sha256"], "Static sender changed; rerun scripts/verify_artifacts.py"
