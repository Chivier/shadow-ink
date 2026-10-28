#!/usr/bin/env python3
"""Render the HTML timeline deterministically into a 1080p H.264 MP4."""
import argparse
import functools
import http.server
import json
from pathlib import Path
import shutil
import signal
import subprocess
import threading
from playwright.sync_api import sync_playwright

signal.signal(signal.SIGINT, signal.default_int_handler)
ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--reference', action='store_true', help='Use the locally prepared reference image')
parser.add_argument('--output', type=Path, default=ROOT / 'output/shadow-ink-demo.mp4')
parser.add_argument('--fps', type=int, default=30, choices=[24, 30, 60])
parser.add_argument('--width', type=int, default=1920)
parser.add_argument('--browser', help='Chromium executable; defaults to Chrome on macOS')
args = parser.parse_args()
if args.width < 640 or args.width % 32:
    parser.error('Width must be at least 640 and a multiple of 32 for a 16:9 H.264 frame.')
if not shutil.which('ffmpeg'):
    parser.error('ffmpeg is required.')
args.output = args.output.resolve()
if args.output.exists():
    parser.error('Output already exists; choose a new path.')
args.output.parent.mkdir(parents=True, exist_ok=True)

class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *unused):
        pass

server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(Quiet, directory=str(ROOT)))
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    with sync_playwright() as p:
        chrome = Path('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome')
        executable = args.browser or (str(chrome) if chrome.is_file() else p.chromium.executable_path)
        browser = p.chromium.launch(executable_path=executable, headless=True)
        try:
            page = browser.new_page(viewport={'width': args.width, 'height': args.width * 9 // 16}, device_scale_factor=1)
            errors = []
            page.on('pageerror', lambda error: errors.append(str(error)))
            query = '?record=1' + ('&reference=1' if args.reference else '')
            page.goto(f'http://127.0.0.1:{server.server_port}/{query}')
            page.wait_for_function('window.DEMO_READY === true', timeout=30000)
            page.evaluate('window.demo.pause()')
            frames = 45 * args.fps
            command = ['ffmpeg', '-hide_banner', '-loglevel', 'warning', '-f', 'image2pipe', '-vcodec', 'mjpeg', '-framerate', str(args.fps), '-i', 'pipe:0', '-an', '-c:v', 'libx264', '-preset', 'medium', '-crf', '18', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', str(args.output)]
            with (args.output.parent / (args.output.stem + '-encode.log')).open('w') as log:
                encoder = subprocess.Popen(command, stdin=subprocess.PIPE, stderr=log)
                try:
                    for frame in range(frames):
                        page.evaluate('(t) => window.demo.seek(t)', frame / args.fps)
                        encoder.stdin.write(page.screenshot(type='jpeg', quality=96, animations='disabled'))
                        if frame % (args.fps * 5) == 0:
                            print(f'Rendering {frame // args.fps:02d}s / 45s', flush=True)
                    encoder.stdin.close()
                    if encoder.wait(timeout=120):
                        raise RuntimeError('ffmpeg failed; inspect the encode log')
                finally:
                    if encoder.poll() is None:
                        encoder.kill()
                        encoder.wait()
            if errors:
                raise RuntimeError(errors)
            report = {'output': str(args.output), 'seconds': 45, 'fps': args.fps, 'frames': frames, 'width': args.width, 'height': args.width * 9 // 16, 'source_sha256': page.evaluate('window.demo.payload.sha256'), 'scope': 'Deterministic HTML visualization; not a throughput benchmark.'}
            args.output.with_suffix('.json').write_text(json.dumps(report, indent=2) + '\n')
            print(json.dumps(report, indent=2))
        finally:
            browser.close()
finally:
    server.shutdown()
    server.server_close()
