#!/usr/bin/env python3
"""Refresh the documentation's verbatim CLI help and check one-command coverage."""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', default=shutil.which('shadow-ink') or str(ROOT / 'target/release/shadow-ink'))
args = parser.parse_args()
commands = ['get', 'prepare', 'send', 'recv', 'status', 'inspect', 'doctor', 'panes', 'calibrate']
help_text = {name: subprocess.check_output([args.binary, name, '--help'], text=True) for name in commands}
version = subprocess.check_output([args.binary, '--version'], text=True).strip().split()[-1]
expected = re.search(r'^version = "([^"]+)"', (ROOT / 'Cargo.toml').read_text(), re.M).group(1)
if version != expected:
    raise SystemExit(f'Expected CLI v{expected}, got v{version}. Build current source or pass --binary.')
parameters = [
    {'flag': '--backend', 'value': 'pipe | text | qr', 'default': 'pipe', 'description': 'Choose a Base64 stream, sampled text, or QR playback.'},
    {'flag': '-o, --output', 'value': 'DIRECTORY', 'default': '~/Downloads/from-SERVER/', 'description': 'Choose the local destination directory.'},
    {'flag': '--resume', 'value': '', 'default': 'automatic for unfinished jobs', 'description': 'Require the saved snapshot. Fails if no local record exists; mutually exclusive with --fresh.'},
    {'flag': '--fresh', 'value': '', 'default': 'off', 'description': 'Prepare a new snapshot of the current source, keeping previous snapshots and state.'},
    {'flag': '--speed', 'value': 'auto | safe | balanced | fast | turbo', 'default': 'auto in QR mode', 'description': 'Select a QR profile. Use --fps for text and pipe instead.'},
    {'flag': '--fps', 'value': '0.1–60', 'default': '30 for pipe/text; QR profile', 'description': 'Limit sender batches or screens per second. This setting is not measured throughput.'},
    {'flag': '--capture-fps', 'value': '0.1–120', 'default': '120 for pipe/text; QR profile', 'description': 'Limit local capture polling per second.'},
    {'flag': '--timeout', 'value': 'SECONDS > 0', 'default': 'no receive time limit', 'description': 'Stop this reception after the time limit and retain its checkpoint.'},
    {'flag': '--max-bytes', 'value': 'BYTES > 0', 'default': '8589934592 (8 GiB)', 'description': 'Limit both the original file and transport object. Manual recv defaults to 1 GiB.'},
    {'flag': '--no-ui', 'value': '', 'default': 'automatic UI selection', 'description': 'Use a background tmux canvas and print progress in the current shell.'},
    {'flag': '--fixed-rate', 'value': '', 'default': 'off', 'description': 'Keep the requested sending rate without adaptive slowdown.'},
    {'flag': '--benchmark', 'value': '', 'default': 'off', 'description': 'Benchmark QR profiles for a host and store complete, verified recommendations.'},
    {'flag': '--list-speeds', 'value': '', 'default': 'off', 'description': 'List available QR speed profiles without starting a transfer.'},
    {'flag': '-h, --help', 'value': '', 'default': '', 'description': 'Print the one-command help.'},
]
actual = set(re.findall(r'--[a-z][a-z-]*', help_text['get']))
covered = set(re.findall(r'--[a-z][a-z-]*', ' '.join(p['flag'] for p in parameters)))
if actual != covered:
    raise SystemExit(f'Update parameter descriptions: undocumented={actual-covered}, stale={covered-actual}')
data = {'version': version, 'parameters': parameters, 'commands': help_text}
(ROOT / 'docs/cli-reference.json').write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')
markdown = f'# CLI reference\n\nVerbatim help from `shadow-ink {version}`. Refresh with `python3 scripts/update-cli-reference.py`.\n'
for name, value in help_text.items():
    markdown += f'\n## {name}\n\n```text\n{value.rstrip()}\n```\n'
(ROOT / 'docs/CLI.md').write_text(markdown)
print(f'Updated {len(commands)} command references and checked {len(parameters)} options for v{version}')
