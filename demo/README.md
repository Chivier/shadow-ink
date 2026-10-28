# Through the terminal

A 45-second, 16:9 film built with static HTML, Canvas, and JavaScript. No build step, CDN, analytics, or JavaScript dependencies. Fonts are self-hosted with their SIL Open Font Licenses in `assets/fonts/`.

```sh
python3 -m http.server 8766 --bind 127.0.0.1 --directory demo
# Open http://127.0.0.1:8766/
```

The player includes play/pause, replay, seek, chapters, prepared-reference selection, fullscreen, keyboard controls (Space and Left/Right), a transcript, and a reduced-motion default. Add `?autoplay=1` for automatic playback, `?t=20` to seek, or `?record=1` for a clean recording frame. The public sample is an original SVG composition, covered by the repository's MIT license.

## Storyboard

| Time | Scene | On-screen mechanism |
| --- | --- | --- |
| 00–05 | Through the terminal | Source image, immutable snapshot, waiting receiver |
| 05–15 | A file, written in light | QR V12-M, 224-byte shards |
| 15–25 | A denser kind of signal | QR V40-L, 2,893-byte shards |
| 25–36 | Let the characters flow | Real SI1 Base64 envelopes, tmux pipe-pane |
| 36–45 | Every byte, back where it belongs | Restored image, complete source SHA-256 |

Each transport chapter is a separate illustrative pass. Timings, progress, packet dots, and the tiled image reveal are choreographed; they do not measure throughput or emulate the exact receiver's internal scheduling. The frame exporter uses the actual Rust bundle, FEC, sender schedule, CRC32C, QR generator, and SI1 envelope implementation. QR images encode real frame bytes. The film cycles a bounded set of exported frames, rather than transmitting the entire source inside the browser.

## Use a reference image

```sh
python3 demo/prepare.py /path/to/image.png
# Open http://127.0.0.1:8766/?reference=1
```

For a screenshot containing an image, `--crop x,y,width,height` chooses the displayed rectangle. It never changes the transferred file or its checksum. The crop must fit inside the image. The provided source is copied to ignored `assets/reference.png`, and matching frame data is written to ignored `assets/local-payload.js`. Neither is published. Only use images you are entitled to distribute when sharing a rendered film. The original supplied reference image is separate from the code's MIT license.

To regenerate the public sample's real frames:

```sh
cargo run --release --locked --example demo_frames -- demo/assets/sample.svg demo/assets/payload.js
```

## Record a film

Requirements: Python 3.9+, Playwright, FFmpeg, and Chromium or Google Chrome. The transfer validator also needs the full `shadow-ink` executable and tmux in PATH.

```sh
python3 -m pip install playwright
python3 -m playwright install chromium
python3 demo/verify-transfer.py demo/assets/reference.png
python3 demo/record.py --reference --output demo/output/shadow-ink-demo.mp4
```

The recorder launches its own loopback HTTP server and headless browser, advances the HTML timeline frame by frame, and sends the rendered frames to FFmpeg. Output is 1920×1080, 30 fps, H.264, 45 seconds, silent, with fast-start metadata. It refuses to overwrite an existing file. Use `--width 1280` or `--fps 24` for smaller output; `--browser /path/to/chromium` selects a browser. Rendering speed depends on the machine; it does not affect the exported film's duration.

`verify-transfer.py` runs the actual one-command CLI through a real isolated local tmux/PTY/pipe-pane, replacing only the SSH hop with a local command shim. It independently checks the recovered file's SHA-256. It records what was really executed in ignored `output/verification.json`. This verifies a local transport path, not remote SSH bandwidth or browser reconstruction.
