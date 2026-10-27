#!/usr/bin/env python3
"""Validate static server binaries, including SSH-only playback and ARM emulation.

Prerequisites: built musl senders, full native release, local OpenSSH server/keygen,
and an aarch64 user-mode emulator. No system configuration is modified.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
os.chdir(ROOT)
REPORT = ROOT / "acceptance-output"
REPORT.mkdir(exist_ok=True)
FULL = ROOT / "target/release/tqr"
SENDERS = {arch: ROOT / f"target/{arch}-unknown-linux-musl/release/tqr" for arch in ["x86_64", "aarch64"]}
EMULATOR = Path(os.environ.get("TQR_ARM_EMULATOR", str(ROOT / "target/build-tools/qemu/usr/bin/qemu-aarch64-static")))


def checked(command, **kwargs):
    result = subprocess.run([str(s) for s in command], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, **kwargs)
    if result.returncode:
        raise RuntimeError(f"{command}: exit {result.returncode}\n{result.stderr}\n{result.stdout}")
    return result


def acceptance(label, sender, prefix):
    env = os.environ.copy()
    env.update(TQR_ACCEPTANCE_BIN=str(FULL), TQR_ACCEPTANCE_SENDER=str(sender), TQR_ACCEPTANCE_PLAYBACK_PREFIX=json.dumps([str(s) for s in prefix]), TQR_ACCEPTANCE_LABEL=label)
    command = ["cargo", "test", "--locked", "--test", "acceptance", "real_two_pane", "--", "--ignored", "--nocapture"]
    print("ACCEPTANCE", label, flush=True)
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, env=env)
    (REPORT / f"artifacts-{label}.log").write_text(result.stdout)
    print(result.stdout, end="", flush=True)
    if result.returncode:
        raise RuntimeError(f"{label} failed")


artifacts = {}
for arch, sender in SENDERS.items():
    data = sender.read_bytes()
    assert data[:4] == b"\x7fELF"
    assert int.from_bytes(data[18:20], "little") == {"x86_64": 62, "aarch64": 183}[arch]
    assert "INTERP" not in checked(["readelf", "-l", sender]).stdout
    prefix = [] if arch == "x86_64" else [EMULATOR]
    help_text = checked(prefix + [sender, "--help"]).stdout
    assert "prepare" in help_text and "recv" not in help_text
    with tempfile.TemporaryDirectory(prefix="tqr-artifact-") as temp:
        temp = Path(temp)
        source, bundle = temp / "source.txt", temp / "prepared.tqtx"
        content = b"terminal-only sender artifact validation\n" * 2000
        source.write_bytes(content)
        prepared = json.loads(checked(prefix + [sender, "prepare", source, "--compress", "zstd", "--out", bundle]).stdout)
        inspected = json.loads(checked([FULL, "inspect", bundle]).stdout)
        assert prepared["transfer_id"] == inspected["transfer_id"]
        assert inspected["manifest"]["original_sha256"] == hashlib.sha256(content).hexdigest()
        assert inspected["manifest"]["compression"] == "zstd"
    artifacts[arch] = {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data), "static_elf": True, "help_prepare_inspect_passed": True, "execution": "native" if not prefix else "qemu-aarch64 8.2.2"}

# The SSH server accepts only an invocation of this sender's TUI playback.
# No SCP/SFTP subsystem, forwarding, user config or public listening address.
with tempfile.TemporaryDirectory(prefix="tqr-ssh-") as temp:
    temp = Path(temp)
    host, client = temp / "host", temp / "client"
    checked(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", host])
    checked(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", client])
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    known = temp / "known_hosts"
    known.write_text(f"[127.0.0.1]:{port} " + host.with_suffix(".pub").read_text())
    force = temp / "playback_only.py"
    force.write_text("import os,shlex,sys\nargs=shlex.split(os.environ.get('SSH_ORIGINAL_COMMAND',''))\nallowed=" + repr(str(SENDERS["x86_64"])) + "\nif args[:2] != [allowed,'send']: sys.exit(126)\nos.execv(allowed,args)\n")
    config = temp / "sshd_config"
    config.write_text(f"""Port {port}
ListenAddress 127.0.0.1
HostKey {host}
PidFile {temp / 'sshd.pid'}
AuthorizedKeysFile {client.with_suffix('.pub')}
PasswordAuthentication no
KbdInteractiveAuthentication no
UsePAM no
StrictModes no
PermitRootLogin no
PermitTTY yes
DisableForwarding yes
PermitUserEnvironment no
PrintMotd no
PrintLastLog no
LogLevel ERROR
ForceCommand /usr/bin/python3 {force}
""")
    sshd = shutil.which("sshd") or "/usr/sbin/sshd"
    with (temp / "sshd.log").open("w") as log:
        server = subprocess.Popen([sshd, "-D", "-e", "-f", str(config)], stdin=subprocess.DEVNULL, stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 5
            while True:
                if server.poll() is not None:
                    raise RuntimeError((temp / "sshd.log").read_text())
                try:
                    with socket.create_connection(("127.0.0.1", port), timeout=.1):
                        break
                except OSError:
                    if time.monotonic() > deadline:
                        raise RuntimeError("isolated SSH server did not start")
                    time.sleep(.05)
            prefix = ["ssh", "-F", "/dev/null", "-tt", "-p", str(port), "-i", client, "-o", "IdentitiesOnly=yes", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes", "-o", f"UserKnownHostsFile={known}", "127.0.0.1"]
            acceptance("static-x86-ssh", SENDERS["x86_64"], prefix)
        except Exception:
            print((temp / "sshd.log").read_text())
            raise
        finally:
            server.terminate()
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()

acceptance("static-arm-qemu", SENDERS["aarch64"], [EMULATOR])
(REPORT / "artifacts.json").write_text(json.dumps({"all_passed": True, "artifacts": artifacts, "x86_64_ssh_tui_only_end_to_end": True, "aarch64_qemu_tui_end_to_end": True, "scope": "Loopback SSH and local emulator, not the user's real host or display."}, indent=2) + "\n")
print("Static artifact acceptance passed", flush=True)
