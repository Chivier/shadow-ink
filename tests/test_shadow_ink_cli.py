"""Exercise the installed CLI and real tmux, with a local stand-in for SSH."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import shlex
import signal
import sqlite3
import time
import subprocess
import tempfile
import unittest

BINARY = shutil.which('shadow-ink')


@unittest.skipUnless(BINARY and shutil.which('tmux'), 'requires full shadow-ink and tmux')
class OneCommand(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='shadow-ink-cli-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        fakebin = self.root/'bin'
        fakebin.mkdir()
        # Preserve the real PTY/tmux path. Only the SSH network hop is replaced.
        ssh = fakebin/'ssh'
        ssh.write_text('#!/usr/bin/env python3\nimport os,sys,json\nwith open(os.environ["SSH_TEST_LOG"], "a") as out: out.write(json.dumps(sys.argv[-1])+"\\n")\nos.execv("/bin/sh", ["sh", "-c", sys.argv[-1]])\n')
        ssh.chmod(0o755)
        self.env = dict(os.environ, SSH_TEST_LOG=str(self.root/'ssh.jsonl'), HOME=str(self.root), XDG_STATE_HOME=str(self.root/'state'),
                        PATH=str(fakebin)+os.pathsep+os.environ['PATH'])
        self.output = self.root/'received'

    def run_cli(self, *args, success=True):
        result = subprocess.run([BINARY, *map(str, args), '-o', str(self.output), '--no-ui'],
                                env=self.env, capture_output=True, text=True, timeout=60)
        if success:
            self.assertEqual(result.returncode, 0, result.stdout+'\n'+result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0)
        return result

    def test_pipe_timeout_resume_hash_and_literal_path(self):
        source = self.root/"随机 $(touch BAD) ' data.bin"
        content = os.urandom(2*1024*1024)
        source.write_bytes(content)
        first = self.run_cli('example', source, '--fps', '10', '--timeout', '.4', success=False)
        self.assertIn('进度已保存', first.stderr)
        latest = next((self.root/'state/shadow-ink-get').glob('*/latest.json'))
        identity = json.loads(latest.read_text())['transfer_id']
        self.run_cli('example', source)
        self.assertEqual(json.loads(latest.read_text())['transfer_id'], identity)
        self.assertEqual(hashlib.sha256((self.output/source.name).read_bytes()).digest(), hashlib.sha256(content).digest())
        self.assertFalse((self.root/'BAD').exists())
        # Existing identical output is independently verified and returned.
        self.assertIn('SHA-256 一致', self.run_cli('example', source).stdout)

    def test_sigint_targeted_resume_and_corrupt_completed_group(self):
        source = self.root/'resume.bin'
        original = os.urandom(4*1024*1024)
        source.write_bytes(original)
        with (self.root/'first.log').open('w') as log:
            proc = subprocess.Popen([BINARY, 'example', str(source), '-o', str(self.output), '--no-ui'],
                                    env=self.env, stdout=log, stderr=log)
            try:
                deadline = time.monotonic()+30
                completed = []
                while time.monotonic() < deadline and proc.poll() is None:
                    databases = list((self.root/'state/shadow-ink-get').glob('*/*/state/state.sqlite'))
                    if databases:
                        try:
                            with sqlite3.connect(databases[0]) as db:
                                completed = [row[0] for row in db.execute('SELECT g FROM completed')]
                        except sqlite3.OperationalError:
                            pass
                        if len(completed) >= 2:
                            break
                    time.sleep(.02)
                self.assertGreaterEqual(len(completed), 2, (self.root/'first.log').read_text())
                proc.send_signal(signal.SIGINT)
                proc.wait(timeout=15)
                self.assertEqual(proc.returncode, 130)
            finally:
                if proc.poll() is None:
                    proc.kill()
                    proc.wait()
        latest = next((self.root/'state/shadow-ink-get').glob('*/latest.json'))
        meta = json.loads(latest.read_text())
        job = latest.parent/meta['transfer_id']
        with sqlite3.connect(job/'state/state.sqlite') as db:
            completed = [row[0] for row in db.execute('SELECT g FROM completed ORDER BY g')]
        self.assertGreaterEqual(len(completed), 2)
        # Force recovery to discover damage that stale status counters would skip.
        corrupt = completed[0]
        with (job/'state'/(meta['transfer_id']+'.part')).open('r+b') as part:
            part.seek(corrupt*(64*meta['manifest']['symbol_bytes']-36))
            old = part.read(1)
            part.seek(-1, 1)
            part.write(bytes([old[0] ^ 1]))
        source.write_bytes(b'changed after interruption')
        result = self.run_cli('example', source, '--resume', '--fps', '30')
        self.assertIn('断点校验完成', result.stdout)
        report = json.loads((job/'result.json').read_text())
        checkpoint = report['resume_checkpoint']
        self.assertGreater(checkpoint['completed_generations'], 0)
        self.assertLess(checkpoint['completed_generations'], checkpoint['total_generations'])
        replay = set()
        for item in report['replay_generations'].split(','):
            ends = item.split('-')
            replay.update(range(int(ends[0]), int(ends[-1])+1))
        self.assertIn(corrupt, replay)
        self.assertNotIn(completed[1], replay)
        commands = [json.loads(line) for line in (self.root/'ssh.jsonl').read_text().splitlines()]
        sender = shlex.split(next(command for command in reversed(commands) if '--only-generations' in command))
        self.assertEqual(sender[sender.index('--only-generations')+1], checkpoint['missing_generations'])
        self.assertEqual((self.output/source.name).read_bytes(), original)
        self.assertEqual(json.loads(latest.read_text())['transfer_id'], meta['transfer_id'])

    def test_explicit_resume_requires_record_and_conflicts_with_fresh(self):
        source = self.root/'not-started.bin'
        source.write_bytes(b'content')
        self.assertIn('没有可续传', self.run_cli('example', source, '--resume', success=False).stderr)
        self.assertFalse((self.root/'ssh.jsonl').exists())
        self.run_cli('example', source, '--resume', '--fresh', success=False)

    def test_screen_backend_and_directory_compression(self):
        source = self.root/'screen.bin'
        content = os.urandom(128*1024)
        source.write_bytes(content)
        self.run_cli('get', 'example', source, '--backend', 'text')
        self.assertEqual((self.output/source.name).read_bytes(), content)
        folder = self.root/'folder'
        folder.mkdir()
        (folder/'notes.txt').write_bytes(b'hello'*10000)
        self.run_cli('example:'+str(folder))
        import tarfile
        with tarfile.open(self.output/'folder.tar') as archive:
            self.assertEqual(archive.extractfile('folder/notes.txt').read(), b'hello'*10000)

    def test_sender_exits_when_terminal_master_closes(self):
        import fcntl
        import pty
        import select
        import struct
        import termios
        source = self.root/'hangup.bin'
        source.write_bytes(os.urandom(4096))
        bundle = self.root/'hangup.tqtx'
        subprocess.run([BINARY, 'prepare', str(source), '--out', str(bundle),
                        '--symbol-bytes', '165'], check=True, capture_output=True)
        for backend in ('qr', 'text', 'text-stream'):
            with self.subTest(backend=backend):
                master, slave = pty.openpty()
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 120, 320, 0, 0))
                proc = subprocess.Popen([BINARY, 'send', str(bundle), '--backend', backend, '--fps', '30', '--loop'],
                                        stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
                os.close(slave)
                try:
                    displayed = b''
                    marker = b'TQ1 ' if backend == 'qr' else b'SI1 '
                    deadline = time.monotonic()+10
                    while marker not in displayed and time.monotonic() < deadline:
                        if select.select([master], [], [], .1)[0]:
                            displayed += os.read(master, 65536)
                    self.assertIn(marker, displayed, 'sender did not begin playback')
                    os.close(master)
                    master = None
                    try:
                        proc.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        self.fail('sender remained alive after PTY hangup: '+backend)
                finally:
                    if master is not None:
                        os.close(master)
                    if proc.poll() is None:
                        proc.kill()
                        proc.wait()

    def test_new_entrypoint_help_and_bad_hosts(self):
        help_result = subprocess.run([BINARY, 'get', '--help'], env=self.env, capture_output=True, text=True)
        self.assertEqual(help_result.returncode, 0)
        self.assertIn('--backend', help_result.stdout)
        self.run_cli('host;touch BAD', '/tmp/file', success=False)
        self.assertFalse((self.root/'BAD').exists())


if __name__ == '__main__':
    unittest.main()
