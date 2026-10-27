#!/usr/bin/env python3
"""Run all first-release gates and retain command results and source provenance."""
import json
import os
from pathlib import Path
import subprocess
import time
from source_fingerprint import source_fingerprint

ROOT = Path(__file__).resolve().parents[1]
os.chdir(ROOT)
REPORT = ROOT / "acceptance-output"
REPORT.mkdir(exist_ok=True)
source_hash = source_fingerprint(ROOT)
checks = []
commands = [
    ["cargo", "fmt", "--check"],
    ["cargo", "test", "--locked"],
    ["cargo", "test", "--locked", "--no-default-features"],
    ["cargo", "clippy", "--locked", "--all-targets", "--all-features", "--", "-D", "warnings"],
    ["cargo", "clippy", "--locked", "--no-default-features", "--all-targets", "--", "-D", "warnings"],
    ["cargo", "build", "--release", "--locked"],
    ["cargo", "test", "--locked", "--test", "acceptance", "--", "--ignored", "--nocapture", "--test-threads=1"],
]
for number, command in enumerate(commands):
    print("RUN", " ".join(command), flush=True)
    start = time.monotonic()
    env = os.environ.copy()
    env["TQR_ACCEPTANCE_BIN"] = str(ROOT / "target/release/tqr")
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, env=env)
    log = REPORT / f"gate-{number + 1}.log"
    log.write_text(result.stdout)
    print(result.stdout, end="", flush=True)
    checks.append({"command": command, "exit_code": result.returncode, "seconds": round(time.monotonic() - start, 3), "log": log.name})
    (REPORT / "checks.json").write_text(json.dumps({"source_sha256": source_hash, "all_passed": len(checks) == len(commands) and all(c["exit_code"] == 0 for c in checks), "checks": checks}, indent=2) + "\n")
    if result.returncode:
        raise SystemExit(result.returncode)
print("All gates passed; reports:", REPORT, flush=True)
