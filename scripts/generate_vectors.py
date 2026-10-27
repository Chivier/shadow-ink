#!/usr/bin/env python3
"""Independent GF(256) implementation for the checked-in wire compatibility fixture.

This is an offline specification check, never used by production encoding.
"""
import hashlib
import json
import struct
from pathlib import Path


EXP = [0] * 512
LOG = [0] * 256
x = 1
for i in range(255):
    EXP[i] = x
    LOG[x] = i
    x <<= 1
    if x & 256:
        x ^= 0x11D
for i in range(255, 512):
    EXP[i] = EXP[i - 255]


def mul(a, b):
    return EXP[LOG[a] + LOG[b]] if a and b else 0


def inverse(matrix):
    n = len(matrix)
    a = [row[:] + [int(i == j) for j in range(n)] for i, row in enumerate(matrix)]
    for c in range(n):
        pivot = next(r for r in range(c, n) if a[r][c])
        a[c], a[pivot] = a[pivot], a[c]
        coefficient = EXP[255 - LOG[a[c][c]]]
        a[c] = [mul(v, coefficient) for v in a[c]]
        for r in range(n):
            if r != c and a[r][c]:
                coefficient = a[r][c]
                a[r] = [v ^ mul(coefficient, w) for v, w in zip(a[r], a[c])]
    return [row[n:] for row in a]


def generate(k, n, size):
    data = bytes((i * 31) % 256 for i in range(k * size - 36))
    raw = len(data).to_bytes(4, "big") + hashlib.sha256(data).digest() + data
    vandermonde = []
    for r in range(n):
        row, value = [], 1
        for _ in range(k):
            row.append(value)
            value = mul(value, r)
        vandermonde.append(row)
    top_inverse = inverse(vandermonde[:k])
    shards = [raw[i * size:(i + 1) * size] for i in range(k)]
    for r in range(k, n):
        row = []
        for c in range(k):
            value = 0
            for j in range(k):
                value ^= mul(vandermonde[r][j], top_inverse[j][c])
            row.append(value)
        out = bytearray(size)
        for coefficient, shard in zip(row, shards):
            for i, value in enumerate(shard):
                out[i] ^= mul(coefficient, value)
        shards.append(bytes(out))
    return {
        "profile": "tqr-v1-gf256-systematic-vandermonde",
        "polynomial": "0x11d",
        "k": k, "n": n, "symbol_bytes": size,
        "input": "data[i] = (i * 31) mod 256; length = k * symbol_bytes - 36",
        "data_sha256": hashlib.sha256(data).hexdigest(),
        "shard_sha256": [hashlib.sha256(s).hexdigest() for s in shards],
        "all_shards_sha256": hashlib.sha256(b"".join(shards)).hexdigest(),
    }


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1] / "tests" / "vectors"
    root.mkdir(parents=True, exist_ok=True)
    for k, n, size in [(64, 96, 224), (3, 5, 224)]:
        path = root / f"rs-{k}-{n}-{size}.json"
        path.write_text(json.dumps(generate(k, n, size), indent=2) + "\n")
        print(path)
    header = struct.pack('>4sBBBB16sIIHBBHHQQ', b'TQR\0', 1, 1, 1, 0,
                         bytes([42])*16, 1, 7, 3, 64, 96, 224, 224, 123, 456)
    raw = header + bytes(range(224))
    crc = 0xffffffff
    for value in raw:
        crc ^= value
        for _ in range(8):
            crc = (crc >> 1) ^ (0x82f63b78 if crc & 1 else 0)
    raw += (crc ^ 0xffffffff).to_bytes(4, 'big')
    path = root / 'frame-v1.json'
    path.write_text(json.dumps({'frame_hex': raw.hex(), 'frame_sha256': hashlib.sha256(raw).hexdigest()}, indent=2) + '\n')
    print(path)
