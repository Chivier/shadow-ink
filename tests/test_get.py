"""Wrapper regressions; uses the real local shadow-ink to prepare temporary snapshots."""
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from argparse import Namespace
import time

SCRIPT = Path(__file__).resolve().parents[1]/'scripts/shadow-ink-get'
LOADER = importlib.machinery.SourceFileLoader('shadow_ink_get', str(SCRIPT))
SPEC = importlib.util.spec_from_loader(LOADER.name, LOADER)
MODULE = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(MODULE)


class SourceArguments(unittest.TestCase):
    def test_host_option_and_shell_injection_rejected(self):
        for source in ('-oProxyCommand=bad:path', 'host;touch bad:file', 'host:', 'host:file\nnext'):
            with self.subTest(source=source), self.assertRaises(MODULE.GetError):
                MODULE.source_argument(source)

    def test_spaces_tilde_and_metacharacters_are_literal_paths(self):
        path = "~/dir/中文 $(touch BAD) ' file"
        self.assertEqual(MODULE.source_argument('wgkissshot:' + path), ('wgkissshot', path))
        self.assertEqual(MODULE.source_argument(path, kiss=True), ('wgkissshot', path))


class SpeedProfiles(unittest.TestCase):
    def options(self, **changes):
        return Namespace(**dict(dict(speed=None, fps=None, capture_fps=None), **changes))

    def test_geometry_covers_maximum_qr(self):
        self.assertEqual(MODULE.required_geometry(MODULE.SPEEDS['safe']), (74, 40))
        self.assertEqual(MODULE.required_geometry(MODULE.SPEEDS['balanced']), (114, 60))
        self.assertEqual(MODULE.required_geometry(MODULE.SPEEDS['turbo']), (186, 96))

    def test_resume_preserves_capacity_and_requires_fresh_for_changes(self):
        previous = dict(manifest=dict(symbol_bytes=224))
        with tempfile.TemporaryDirectory() as temp:
            chosen = MODULE.choose_settings(self.options(fps=20), 'host', Path(temp), previous)
            self.assertEqual(chosen['symbol_bytes'], 224)
            self.assertEqual(chosen['fps'], 20)
            with self.assertRaises(MODULE.GetError):
                MODULE.choose_settings(self.options(speed='fast'), 'host', Path(temp), previous)
            previous = dict(manifest=dict(symbol_bytes=2893), transfer_settings=dict(MODULE.SPEEDS['fast']))
            chosen = MODULE.choose_settings(self.options(speed='turbo'), 'host', Path(temp), previous)
            self.assertEqual(chosen['fps'], 60)

    def test_auto_uses_recent_matching_measurement_only(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            binary = base/'shadow-ink'
            binary.write_bytes(b'version one')
            target = base/'benchmarks/host/recommended.json'
            target.parent.mkdir(parents=True)
            measured = dict(settings=MODULE.SPEEDS['balanced'], measured_at=time.time(),
                            receiver_sha256=MODULE.binary_hash(binary))
            MODULE.save_json(target, measured)
            with patch.object(MODULE.shutil, 'which', return_value=str(binary)):
                self.assertEqual(MODULE.choose_settings(self.options(), 'host', base)['symbol_bytes'], 712)
                binary.write_bytes(b'version two')
                self.assertEqual(MODULE.choose_settings(self.options(), 'host', base)['symbol_bytes'], 224)
                measured['receiver_sha256'] = MODULE.binary_hash(binary)
                measured['measured_at'] -= 15*86400
                MODULE.save_json(target, measured)
                self.assertEqual(MODULE.choose_settings(self.options(), 'host', base)['symbol_bytes'], 224)

    def test_character_default_and_explicit_backend_on_resume(self):
        args = self.options()
        args.backend = None
        with tempfile.TemporaryDirectory() as temp:
            chosen = MODULE.choose_settings(args, 'host', Path(temp))
            self.assertEqual(chosen['backend'], 'pipe')
            previous = dict(manifest=dict(symbol_bytes=2893), transfer_settings=chosen)
            self.assertEqual(MODULE.choose_settings(args, 'host', Path(temp), previous)['backend'], 'pipe')
            args.backend = 'text'
            with self.assertRaises(MODULE.GetError):
                MODULE.choose_settings(args, 'host', Path(temp), previous)
            args.backend = 'qr'
            args.speed = 'fast'
            self.assertNotIn('backend', MODULE.choose_settings(args, 'host', Path(temp), previous))

    def test_recommendation_requires_two_verified_runs_and_uses_slower_result(self):
        trials = [dict(settings=dict(fps=60), verified=True, verified_bytes_per_second=1000),
                  dict(settings=dict(fps=60), verified=True, verified_bytes_per_second=100),
                  dict(settings=dict(fps=20), verified=True, verified_bytes_per_second=200),
                  dict(settings=dict(fps=20), verified=True, verified_bytes_per_second=210),
                  dict(settings=dict(fps=30), verified=False, verified_bytes_per_second=9000)]
        self.assertEqual(MODULE.measured_recommendation(trials), (dict(fps=20), 200))
        with self.assertRaises(MODULE.GetError):
            MODULE.measured_recommendation(trials[:1])


class Installer(unittest.TestCase):
    def test_install_is_repeatable_and_alias_uses_remote_path_help(self):
        with tempfile.TemporaryDirectory(prefix='shadow-ink-get-install-') as temp:
            destination = Path(temp)/'bin'
            env = dict(os.environ, TQR_GET_BIN_DIR=str(destination))
            installer = SCRIPT.parent/'install-get.sh'
            for _ in range(2):
                result = subprocess.run(['sh', str(installer)], env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((destination/'shadow-ink-get').read_bytes(), SCRIPT.read_bytes())
            self.assertEqual(os.readlink(destination/'kiss-get'), 'shadow-ink-get')
            help_result = subprocess.run([str(destination/'kiss-get'), '--help'], capture_output=True, text=True)
            self.assertEqual(help_result.returncode, 0, help_result.stderr)
            self.assertIn('远程路径（kiss-get）', help_result.stdout)

    def test_existing_unrelated_alias_is_preserved(self):
        with tempfile.TemporaryDirectory(prefix='shadow-ink-get-install-') as temp:
            destination = Path(temp)
            alias = destination/'kiss-get'
            alias.write_text('keep this original')
            result = subprocess.run(['sh', str(SCRIPT.parent/'install-get.sh')],
                                    env=dict(os.environ, TQR_GET_BIN_DIR=temp), capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(alias.read_text(), 'keep this original')
            self.assertFalse((destination/'shadow-ink-get').exists())


@unittest.skipUnless(shutil.which('shadow-ink'), 'install the full shadow-ink binary first')
class RemotePreparation(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='shadow-ink-get-regression-')
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name)
        self.env = dict(os.environ, HOME=str(self.home))

    def prepare(self, path, resume=None, fresh=False, success=True, symbol_bytes=224, compression='auto'):
        config = json.dumps(dict(path=str(path), resume=resume, fresh=fresh, symbol_bytes=symbol_bytes, compress=compression))
        result = subprocess.run([sys.executable, '-', config], input=MODULE.REMOTE_PREPARE,
                                capture_output=True, text=True, env=self.env, timeout=20)
        if not success:
            self.assertNotEqual(result.returncode, 0)
            return result
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout.split('TQR_GET_JSON=')[1])

    def test_stable_cache_and_explicit_new_snapshot(self):
        source = self.home/'sample.bin'
        source.write_bytes(b'original payload')
        first = self.prepare(source)
        self.assertEqual(first['transfer_id'], self.prepare(source)['transfer_id'])
        source.write_bytes(b'changed payload')
        resumed = self.prepare(source, resume=first)
        self.assertEqual(resumed['transfer_id'], first['transfer_id'])
        self.assertTrue(resumed['resumed_snapshot'])
        new = self.prepare(source, fresh=True)
        self.assertNotEqual(new['transfer_id'], first['transfer_id'])
        self.assertNotEqual(new['manifest']['original_sha256'], first['manifest']['original_sha256'])

    def test_relative_and_tilde_paths_use_remote_home(self):
        (self.home/'input.bin').write_bytes(b'payload')
        first = self.prepare('input.bin')
        self.assertEqual(first['transfer_id'], self.prepare('~/input.bin')['transfer_id'])

    def test_different_capacities_do_not_reuse_the_wrong_bundle(self):
        source = self.home/'input.bin'
        source.write_bytes(bytes(range(256))*4)
        small = self.prepare(source)
        large = self.prepare(source, symbol_bytes=2893)
        self.assertEqual(large['manifest']['symbol_bytes'], 2893)
        self.assertNotEqual(small['transfer_id'], large['transfer_id'])
        self.assertEqual(small['transfer_id'], self.prepare(source)['transfer_id'])

    def test_benchmark_samples_are_two_real_rs_generations(self):
        for size in (224, 712, 2893):
            source = self.home/f'benchmark-{size}.bin'
            source.write_bytes(b'x'*MODULE.benchmark_bytes(size))
            meta = self.prepare(source, symbol_bytes=size, compression='none')
            inspected = subprocess.run(['shadow-ink', 'inspect', meta['remote_bundle']],
                                       capture_output=True, text=True, check=True)
            self.assertEqual(json.loads(inspected.stdout)['generations'], 2)

    def test_literal_shell_metacharacters_and_unicode(self):
        source = self.home/"中文 $(touch BAD) ' file.bin"
        source.write_bytes(b'payload')
        meta = self.prepare(source)
        self.assertEqual(meta['manifest']['filename'], source.name)
        self.assertFalse((self.home/'BAD').exists())

    def test_directory_snapshot_and_changed_child(self):
        folder = self.home/'a folder'
        folder.mkdir()
        child = folder/'中文.txt'
        child.write_text('hello')
        (folder/'link').symlink_to(child.name)
        first = self.prepare(folder)
        self.assertEqual(first['manifest']['filename'], 'a folder.tar')
        self.assertEqual(first['source_kind'], 'directory')
        self.assertFalse((Path(first['remote_bundle']).parent/'a folder.tar').exists())
        child.write_text('different')
        self.assertNotEqual(first['transfer_id'], self.prepare(folder)['transfer_id'])

    def test_missing_snapshot_requires_explicit_fresh(self):
        source = self.home/'input.bin'
        source.write_bytes(b'data')
        first = self.prepare(source)
        Path(first['remote_bundle']).unlink()
        result = self.prepare(source, resume=first, success=False)
        self.assertIn('--fresh', result.stderr)

    def test_self_archiving_and_special_files_are_rejected(self):
        result = self.prepare(self.home, success=False)
        self.assertIn('contains the shadow-ink-get cache', result.stderr)
        folder = self.home/'folder'
        folder.mkdir()
        os.mkfifo(folder/'fifo')
        result = self.prepare(folder, success=False)
        self.assertIn('socket, device or FIFO', result.stderr)


if __name__ == '__main__':
    unittest.main()
