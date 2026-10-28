#!/usr/bin/env python3
"""Prepare a local reference image and real frames without committing that image."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('image', type=Path)
parser.add_argument('--crop', help='Display crop x,y,width,height; payload bytes stay unchanged')
args = parser.parse_args()
image = args.image.resolve(strict=True)
if image.suffix.lower() != '.png':
    parser.error('Use a PNG reference image; no conversion is performed.')
crop = None
if args.crop:
    crop = [int(value) for value in args.crop.split(',')]
    if len(crop) != 4 or min(crop[:2]) < 0 or min(crop[2:]) <= 0:
        parser.error('Crop must be x,y,width,height with nonnegative origin and positive size.')
    header = image.read_bytes()[:24]
    if header[:8] != b'\x89PNG\r\n\x1a\n':
        parser.error('Input is not a PNG file.')
    width, height = int.from_bytes(header[16:20], 'big'), int.from_bytes(header[20:24], 'big')
    if crop[0] + crop[2] > width or crop[1] + crop[3] > height:
        parser.error('Crop must fit inside the image.')
source = ROOT / 'assets/reference.png'
if image != source:
    shutil.copyfile(image, source)
output = ROOT / 'assets/local-payload.js'
subprocess.run(['cargo', 'run', '--release', '--locked', '--example', 'demo_frames', '--', str(source), str(output)], cwd=ROOT.parent, check=True)
if crop:
    prefix = 'window.SHADOW_INK_PAYLOAD = '
    data = json.loads(output.read_text()[len(prefix):].strip().removesuffix(';'))
    data['crop'] = crop
    output.write_text(prefix + json.dumps(data, separators=(',', ':')) + ';\n')
print('Ready: http://127.0.0.1:8766/?reference=1')
