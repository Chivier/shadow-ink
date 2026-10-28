#!/usr/bin/env python3
"""Verify the demo image through the real CLI and isolated local tmux.

Only SSH is replaced with a local command shim. PTY, pipe-pane, protocol frames,
CRC, recovery, output publication and the independent SHA-256 check are real.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('image', type=Path, nargs='?', default=ROOT / 'assets/sample.svg')
args = parser.parse_args()
source = args.image.resolve(strict=True)
output = ROOT / 'output'
output.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix='shadow-ink-demo-verify-') as temporary:
    work = Path(temporary)
    tools = work / 'bin'
    tools.mkdir()
    ssh = tools / 'ssh'
    ssh.write_text('#!/usr/bin/env python3\nimport os,sys\nos.execv("/bin/sh", ["sh", "-c", sys.argv[-1]])\n')
    ssh.chmod(0o755)
    env = dict(os.environ, HOME=str(work), XDG_STATE_HOME=str(work / 'state'), PATH=str(tools) + os.pathsep + os.environ['PATH'])
    result = subprocess.run(['shadow-ink', 'demo-local', str(source), '--no-ui', '--timeout', '30', '-o', str(work / 'received')], env=env, capture_output=True, text=True, timeout=60)
    (output / 'verify-transfer.log').write_text(result.stdout + '\n' + result.stderr)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    recovered = work / 'received' / source.name
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    assert hashlib.sha256(recovered.read_bytes()).hexdigest() == digest
    job = next((work / 'state/shadow-ink-get').glob('*/*/result.json'))
    receiver = json.loads(job.read_text())
    assert receiver['complete'] and receiver['status']['finalized']
    report = {'verified': True, 'source_bytes': source.stat().st_size, 'sha256': digest, 'transport': 'real local tmux pipe-pane; local stand-in for SSH', 'receiver': receiver}
    (output / 'verification.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({key: value for key, value in report.items() if key != 'receiver'}, indent=2))
