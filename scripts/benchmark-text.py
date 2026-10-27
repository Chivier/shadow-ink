#!/usr/bin/env python3
"""Real local tmux text transport benchmark. No network throughput claim."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/shadow-ink'))
    parser.add_argument('--sender-binary', type=Path, help='Use a separately built sender')
    parser.add_argument('--sender-prefix', default='[]', help='JSON argument list, e.g. an emulator or SSH prefix')
    parser.add_argument('--mib', type=int, default=32)
    parser.add_argument('--backend', choices=['pipe', 'text'], default='pipe')
    parser.add_argument('--report', type=Path, default=Path('acceptance-output/text.json'))
    args = parser.parse_args()
    binary = str(args.binary.resolve())
    binary_hash = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
    sender = str((args.sender_binary or args.binary).resolve())
    sender_hash = hashlib.sha256(Path(sender).read_bytes()).hexdigest()
    prefix = json.loads(args.sender_prefix)
    if not isinstance(prefix, list) or not all(isinstance(value, str) for value in prefix):
        parser.error('--sender-prefix must be a JSON list of strings')
    with tempfile.TemporaryDirectory(prefix='shadow-ink-bench-') as temporary:
        root = Path(temporary)
        source = root/'random.bin'
        digest = hashlib.sha256()
        with source.open('wb') as out:
            for _ in range(args.mib):
                block = os.urandom(1024*1024)
                digest.update(block)
                out.write(block)
        bundle = root/'bundle.tqtx'
        subprocess.run([binary, 'prepare', str(source), '--out', str(bundle), '--compress', 'none',
                        '--symbol-bytes', '2893' if args.backend == 'pipe' else '165'], check=True, stdout=subprocess.DEVNULL)
        socket = str(root/'tmux.sock')
        env = dict(os.environ, TERM='dumb')
        env.pop('TMUX', None)
        def tmux(*command):
            return subprocess.check_output(['tmux', '-S', socket, '-f', '/dev/null', *command], env=env, text=True).strip()
        pane = tmux('new-session', '-d', '-P', '-F', '#{pane_id}', '-x', '320', '-y', '120', 'sleep 86400')
        receiver = None
        try:
            tmux('set-option', '-g', 'status', 'off')
            tmux('set-option', '-g', 'remain-on-exit', 'on')
            state = root/'state'
            started = time.monotonic()
            with (root/'result.json').open('w') as out, (root/'progress.log').open('w') as log:
                receiver = subprocess.Popen([binary, 'recv', '--source', args.backend + ':' + pane,
                    '--tmux-socket', socket, '--output', str(root/'received'), '--state', str(state),
                    '--capture-fps', '120', '--timeout', '180'], stdout=out, stderr=log)
                if args.backend == 'pipe':
                    deadline = time.monotonic()+10
                    while not (state/'pipe-ready').exists():
                        if receiver.poll() is not None or time.monotonic() > deadline:
                            raise RuntimeError((root/'progress.log').read_text())
                        time.sleep(.01)
                command = shlex.join(prefix + [sender, 'send', str(bundle), '--backend',
                    'text-stream' if args.backend == 'pipe' else 'text', '--fps', '30', '--loop'])
                tmux('respawn-pane', '-k', '-t', pane, command)
                receiver.wait(timeout=190)
            elapsed = time.monotonic()-started
            result = json.loads((root/'result.json').read_text())
            if receiver.returncode or not result['complete']:
                raise RuntimeError((root/'progress.log').read_text()[-4000:] + '\n' + json.dumps(result))
            actual = hashlib.sha256((root/'received/random.bin').read_bytes()).hexdigest()
            assert actual == digest.hexdigest()
            assert hashlib.sha256(Path(binary).read_bytes()).hexdigest() == binary_hash, 'binary changed during benchmark; rerun with a stable build'
            assert hashlib.sha256(Path(sender).read_bytes()).hexdigest() == sender_hash, 'sender changed during benchmark'
            report = dict(sender_sha256=sender_hash, sender_prefix=prefix, scope='local real tmux PTY; no SSH/network or four-hour stability claim',
                backend=args.backend, bytes=source.stat().st_size, sha256=actual, verified=True,
                wall_seconds=elapsed, verified_bytes_per_second=source.stat().st_size/elapsed,
                receiver=result, binary_sha256=binary_hash)
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps(report, indent=2)+'\n')
            print(json.dumps({k: v for k,v in report.items() if k != 'receiver'}, indent=2))
        finally:
            if receiver and receiver.poll() is None:
                receiver.terminate()
                receiver.wait(timeout=10)
            tmux('kill-server')


if __name__ == '__main__':
    main()
