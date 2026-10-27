#!/usr/bin/env python3
"""Build full macOS receivers, ad-hoc sign, inspect, and package thin/universal2."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import sys
import tomllib
from macos_binary import inspect
from source_fingerprint import source_fingerprint

ROOT = Path(__file__).resolve().parents[1]
os.chdir(ROOT)
REPORT = ROOT / "acceptance-output"
REPORT.mkdir(exist_ok=True)
subprocess.run([sys.executable, "scripts/package_ready.py"], check=True)
env = {k: v for k, v in os.environ.items() if not k.startswith("RCODESIGN_")}
env["MACOSX_DEPLOYMENT_TARGET"] = "11.0"
signer = env.get("TQR_RCODESIGN", "rcodesign")
command = ["cargo", "zigbuild", "--release", "--locked", "--features", "receiver", "--target", "universal2-apple-darwin"]
with (REPORT / "macos-build.log").open("w") as log:
    result = subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT)
if result.returncode:
    print((REPORT / "macos-build.log").read_text())
    raise SystemExit(result.returncode)
version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
DIST = ROOT / "dist"
DIST.mkdir(exist_ok=True)
notices = DIST / "THIRD_PARTY_NOTICES.txt"
subprocess.run([sys.executable, "scripts/collect_notices.py", str(notices)], check=True)
artifacts = {}
for arch in ["aarch64", "x86_64", "universal2"]:
    print("PACKAGE macOS", arch, flush=True)
    name = f"tqr-{version}-macos-{arch}"
    stage = DIST / name
    stage.mkdir(exist_ok=True)
    binary = stage / "tqr"
    shutil.copy2(ROOT / f"target/{arch}-apple-darwin/release/tqr", binary)
    signed = subprocess.run([signer, "sign", "--config-file", "/dev/null", "--timestamp-url", "none", "--digest", "sha256", "--binary-identifier", "org.shadow-ink.tqr", str(binary)], env=env, capture_output=True, text=True)
    (REPORT / f"macos-sign-{arch}.log").write_text(signed.stdout + signed.stderr)
    if signed.returncode:
        raise RuntimeError(signed.stdout + signed.stderr)
    verified = subprocess.run([signer, "verify", "--config-file", "/dev/null", str(binary)], env=env, capture_output=True, text=True)
    (REPORT / f"macos-signature-{arch}.log").write_text(verified.stdout + verified.stderr)
    info = inspect(binary)
    verification_diagnostic = verified.stdout + verified.stderr
    # rcodesign 0.29's verifier tries to parse the intentionally empty CMS in
    # ad-hoc signatures. Preserve that known diagnostic; independently verify
    # every CodeDirectory page below. Apple's verifier runs in native CI.
    diagnostic = "CMS error: missing further values (at position 0)\n"
    known_adhoc_diagnostic = (diagnostic if len(info["slices"]) == 1 else "".join(f"@{i}: {diagnostic}" for i in range(len(info["slices"])))) + "(the verify command is known to be buggy and gives misleading results; we highly recommend using Apple's tooling until this message is removed)\nError: problems reported during verification\n"
    if verified.returncode and verification_diagnostic != known_adhoc_diagnostic:
        raise RuntimeError(verification_diagnostic)
    info["rcodesign_verifier_exit_code"] = verified.returncode
    info["verification_method"] = "independent SHA-256 CodeDirectory page verification; native Apple codesign pending"
    expected = {"aarch64", "x86_64"} if arch == "universal2" else {arch}
    if {s["architecture"] for s in info["slices"]} != expected:
        raise RuntimeError("wrong architecture in release")
    artifacts[arch] = info
    for source in ["README.md", "LICENSE"]:
        shutil.copy2(ROOT / source, stage / source)
    shutil.copy2(notices, stage / notices.name)
    (stage / "docs").mkdir(exist_ok=True)
    for source in ["MACOS.md", "PROTOCOL.md", "BUILD.md", "ACCEPTANCE.md"]:
        shutil.copy2(ROOT / "docs" / source, stage / "docs" / source)
    shutil.copy2(ROOT / "scripts/macos_selftest.py", stage / "selftest.py")
    (stage / "SHA256SUMS").write_text(f"{info['sha256']}  tqr\n" + f"{hashlib.sha256((stage / 'selftest.py').read_bytes()).hexdigest()}  selftest.py\n")
    (stage / "reports").mkdir(exist_ok=True)
    shutil.copy2(REPORT / "checks.json", stage / "reports/linux-checks.json")
    (stage / "reports/macos-binary.json").write_text(json.dumps(info, indent=2) + "\n")
    archive = DIST / f"{name}.tar.gz"
    with tarfile.open(archive, "w:gz") as tar:
        tar.add(stage, arcname=name)
    info["archive"] = archive.name
    info["archive_sha256"] = hashlib.sha256(archive.read_bytes()).hexdigest()
report = {"build_and_structure_checks_passed": True, "native_macos_runtime_verified": False, "source_sha256": source_fingerprint(ROOT), "build_command": command, "macos_deployment_target": "11.0", "sdk_root": Path(env.get("SDKROOT", "unknown")).name, "signing": "SHA-256 ad-hoc; no Developer ID or notarization", "artifacts": artifacts}
(REPORT / "macos.json").write_text(json.dumps(report, indent=2) + "\n")
(DIST / "MACOS-SHA256SUMS").write_text("".join(f"{a['archive_sha256']}  {a['archive']}\n" for a in artifacts.values()))
print(json.dumps(report, indent=2))
