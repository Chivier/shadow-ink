#!/usr/bin/env python3
"""Run a packaged receiver through local tmux playback, zstd and disk resume.

Uses an isolated temporary tmux server; existing sessions are never targeted.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile


def run(command, expected=(0,), timeout=90):
    result = subprocess.run([str(x) for x in command], capture_output=True, text=True, timeout=timeout)
    if result.returncode not in expected:
        raise RuntimeError(f"{command}: exit {result.returncode}\n{result.stderr}\n{result.stdout}")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path(__file__).resolve().parent / "shadow-ink")
    parser.add_argument("--report", type=Path, help="Save the native runtime result here")
    parser.add_argument("--allow-non-macos", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    if sys.platform != "darwin" and not args.allow_non_macos:
        raise RuntimeError("Run this native acceptance test on your Mac")
    binary = args.binary.resolve(strict=True)
    if not shutil.which("tmux"):
        raise RuntimeError("tmux is required: brew install tmux")
    manifest = binary.parent / "SHA256SUMS"
    if manifest.exists():
        for line in manifest.read_text().splitlines():
            want, name = line.split()
            name = name.lstrip("*")
            if Path(name).name != name:
                raise RuntimeError("unsafe checksum entry")
            actual = hashlib.sha256((binary.parent / name).read_bytes()).hexdigest()
            if actual != want:
                raise RuntimeError(f"checksum mismatch: {name}")
    signature_checked = sys.platform == "darwin"
    if signature_checked:
        run(["codesign", "--verify", "--strict", "--all-architectures", binary])
    for command in [[binary, "--help"], [binary, "doctor", "--cell-pixels", "8x16"]]:
        run(command)
    with tempfile.TemporaryDirectory(prefix="shadow-ink-mac-selftest-") as temp:
        temp = Path(temp)
        source, bundle = temp / "native-selftest.bin", temp / "prepared.tqtx"
        content = b"".join(hashlib.sha256(i.to_bytes(4, "big")).digest() for i in range(625))
        source.write_bytes(content)
        prepared = json.loads(run([binary, "prepare", source, "--compress", "zstd", "--out", bundle]).stdout)
        if prepared["manifest"]["compression"] != "zstd":
            raise RuntimeError("compression not enabled")
        socket = temp / "tmux.sock"

        def tmux(*arguments):
            return run(["tmux", "-S", socket, "-f", "/dev/null", *arguments]).stdout.strip()

        try:
            left = tmux("new-session", "-d", "-P", "-F", "#{pane_id}", "-x", "244", "-y", "45", "-s", "shadow-ink-selftest")
            right = tmux("split-window", "-h", "-P", "-F", "#{pane_id}", "-t", left)
            if left == right:
                raise RuntimeError("selftest did not create distinct panes")
            tmux("respawn-pane", "-k", "-t", left, binary, "send", bundle, "--fps", "20", "--loop")
            state, output = temp / "state", temp / "received"
            receive = [binary, "recv", "--source", f"tmux:{left}", "--tmux-socket", socket, "--state", state, "--output", output, "--capture-fps", "60"]
            print("Testing terminal QR playback and saved partial reception...", file=sys.stderr)
            first = json.loads(run(receive + ["--timeout", "1.2"], expected=(2,)).stdout)
            if first["complete"] or first["status"]["cached_symbols"] < 1:
                raise RuntimeError("partial reception was not persisted")
            tmux("respawn-pane", "-k", "-t", left, binary, "send", bundle, "--fps", "20", "--loop")
            print("Restarting playback and resuming the same receiver state...", file=sys.stderr)
            received = json.loads(run(receive + ["--timeout", "60"]).stdout)
            result_bytes = (output / source.name).read_bytes()
            if not received["complete"] or result_bytes != content:
                raise RuntimeError("recovered file differs from source")
            status = json.loads(run([binary, "status", state]).stdout)
            if not status["finalized"]:
                raise RuntimeError("output is not verified")
        finally:
            subprocess.run(["tmux", "-S", str(socket), "kill-server"], capture_output=True)
    # Exercise the exact packaged executable through the public one-command
    # entry point. The network hop is replaced locally; tmux and PTYs are real.
    cli_test = binary.parent / "cli_selftest.py"
    if not cli_test.is_file():
        raise RuntimeError("package is missing cli_selftest.py")
    env = os.environ.copy()
    env["PATH"] = str(binary.parent) + os.pathsep + env["PATH"]
    cli = subprocess.run([sys.executable, str(cli_test), "-v"], env=env,
                         capture_output=True, text=True, timeout=180)
    if cli.returncode:
        raise RuntimeError(cli.stdout + cli.stderr)
    print(cli.stderr, file=sys.stderr)
    report = {"all_passed": True, "native_macos_runtime_verified": sys.platform == "darwin", "platform": platform.platform(), "machine": platform.machine(), "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "codesign_verified": signature_checked, "original_bytes": len(content), "original_sha256": hashlib.sha256(content).hexdigest(), "zstd_and_tmux_resume_verified": True, "text_and_targeted_resume_verified": True, "cli_test_log": cli.stderr, "video_runtime_tested": False, "ffmpeg_available": shutil.which("ffmpeg") is not None}
    if args.report:
        with args.report.open("x") as saved:
            json.dump(report, saved, indent=2)
            saved.write("\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"selftest: {error}", file=sys.stderr)
        raise SystemExit(1)
