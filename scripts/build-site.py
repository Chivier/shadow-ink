#!/usr/bin/env python3
"""Build the static docs site from an explicit set of public repository files."""
import argparse
import html
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, default=ROOT / 'dist/site')
args = parser.parse_args()
output = args.output.resolve()
if output == ROOT or ROOT in output.parents and output.relative_to(ROOT).parts[0] not in ('dist', 'acceptance-output'):
    parser.error('Choose an output directory inside dist/ or acceptance-output/, or outside the repository.')
output.mkdir(parents=True, exist_ok=True)
# Tracked files only: local reference.png, local-payload.js and output videos
# must never enter the public site, even when this runs on a developer machine.
tracked = subprocess.check_output(['git', 'ls-files', '-z', '--', 'docs', 'demo'], cwd=ROOT).decode().split('\0')
owned = ['docs/index.html', 'docs/site.css', 'docs/site.js', 'docs/cli-reference.json', 'docs/CLI.md', 'docs/README.md']
for name in sorted(set(tracked + owned)):
    if not name or not (ROOT / name).is_file():
        continue
    relative = Path(name).relative_to('docs') if name.startswith('docs/') else Path(name)
    target = output / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / name, target)
data = json.loads((ROOT / 'docs/cli-reference.json').read_text())
rows = []
for item in data['parameters']:
    flag = html.escape(item['flag'])
    if item['value']:
        flag += '<br><span class="argument">' + html.escape(item['value']) + '</span>'
    rows.append('<tr><td><code>' + flag + '</code></td><td>' + html.escape(item['default']) + '</td><td>' + html.escape(item['description']) + '</td></tr>')
page = (output / 'index.html').read_text().replace('<!-- PARAMETER_ROWS -->', '\n'.join(rows))
page = page.replace('<!-- CLI_HELP -->', html.escape(data['commands']['get'])).replace('{{VERSION}}', html.escape(data['version']))
(output / 'index.html').write_text(page)
for private in ['demo/assets/reference.png', 'demo/assets/local-payload.js', 'demo/output']:
    if (output / private).exists():
        raise SystemExit(f'Refusing a site directory containing local-only data: {private}. Use a fresh output directory.')
print(f'Built static documentation in {output}')
