# tqr-v1 wire specification

All integers are unsigned big-endian. QR uses a **single byte-mode segment**, fixed version/ECC, no ECI, URL or Base64. The frame header is 56 bytes, followed by exactly `symbol_bytes` payload bytes and a 4-byte big-endian CRC32C. Total overhead: 60 bytes.

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | magic `54 51 52 00` (`TQR\0`) |
| 4 | 1 | version = 1 |
| 5 | 1 | type: 0 manifest, 1 file object |
| 6 | 1 | FEC profile = 1 |
| 7 | 1 | flags = 0; unknown flags rejected |
| 8 | 16 | transfer ID |
| 24 | 4 | object ID, must equal type |
| 28 | 4 | generation ID; manifest must use 0 |
| 32 | 2 | symbol ID, `0 <= symbol < n` |
| 34 | 1 | k |
| 35 | 1 | n |
| 36 | 2 | symbol bytes |
| 38 | 2 | payload length, must equal symbol bytes |
| 40 | 8 | playback run ID, random per sender invocation |
| 48 | 8 | playback sequence, starts at zero, includes manifest frames |
| 56 | S | payload |
| 56+S | 4 | CRC32C over header and payload |

CRC32C uses the Castagnoli polynomial: reflected `0x82f63b78`, initial and final XOR `0xffffffff`. Test: ASCII `123456789` → `e3069283`. It detects accidental corruption, not sender identity.

Symbol size is 64..=2893 bytes (bounded by QR V40-L packet capacity); the sender also checks actual chosen QR capacity. All frames must satisfy `0 < k <= 64`, `k < n <= 96`. Data uses exactly `(k,n)=(64,96)`. Manifest uses the same FEC profile with `k=ceil((manifest_len+36)/S)`, `n=k+ceil(k/2)`, at most one generation. Receiver accepts otherwise valid self-described manifest k/n, preventing bootstrap dependencies. Maximum manifest: 8192 bytes.

## Object and group representation

Manifest is UTF-8 JSON with exactly the fields in `bundle::Manifest`; unknown and duplicate fields are rejected. No floating-point values. Its original byte sequence is persisted and hashed, never reconstructed by JSON reserialization. Transfer ID = first 16 bytes of SHA-256(manifest bytes). Manifest does not contain its own transfer ID, playback ID or sequence.

Required fields:

```json
{
  "version": 1,
  "filename": "results.bin",
  "original_bytes": 0,
  "original_sha256": "<64 lowercase hex characters>",
  "transport_bytes": 0,
  "transport_sha256": "<64 lowercase hex characters>",
  "compression": "none",
  "symbol_bytes": 224,
  "k": 64,
  "n": 96
}
```

Compression is `none` or `zstd`. With `none`, lengths and hashes must match. `prepare --compress auto` selects zstd only if the snapshot compresses smaller. The package freezes the exact selected bytes.

Each generation's K systematic shards concatenate to:

```text
valid_data_len: u32 | SHA256(valid_data): 32 bytes | valid_data | zero padding
```

Group data capacity `C = K*S - 36`. Group `g` holds transport bytes at `[g*C, min((g+1)*C, transport_len))`. Count = `max(1, ceil(transport_len/C))`; an empty file still has one zero-length group. Every data shard has length S, including the final generation. CRC-valid is required before passing shards into erasure recovery. The decoder rejects hash failures, unexpected recovered lengths and nonzero padding.

## FEC profile 1

- Field GF(256), addition XOR, reduction polynomial `x^8+x^4+x^3+x^2+1 = 0x11d`; generator `0x02`.
- `V[r,c] = r^c` in the field, with field elements `r=0..n-1`, `c=0..k-1`; `0^0=1`.
- Let `T=V[0..k,0..k]`. Encoding matrix `G = V * inverse(T)`.
- Shard rows 0..k-1 are systematic data; rows k..n-1 are repair shards. Matrix multiplication is applied independently to each byte offset in S.
- This matches pinned `reed-solomon-erasure 6.0.0` `galois_8::ReedSolomon`. The standalone Python generator and checked-in per-shard SHA-256 vectors pin wire compatibility for RS(64,96) and a manifest-size profile.

An arbitrary set of K correct distinct shards recovers the generation. Repeating a symbol or observing a new playback sequence does not add an independent FEC shard. CRC and group hashes remain necessary; this is erasure recovery, not correction of unknown erroneous shards.

## Playback, framing and reception

The stable shard identity is `(transfer, object, generation, symbol)`. The playback identity is `(run, sequence)` within a transfer. A restart changes run, not transfer. No acknowledgement or dependency on synchronized clocks exists.

Startup sends two complete manifest rotations. Normal playback injects one manifest symbol after every 19 data symbols. Data interleaves windows of at most eight groups, systematic before repair, reshuffling symbol and group order across rounds. Optional generation selection only changes scheduling.

The terminal uses explicit black foreground / white background and half-block cells, four-module quiet zones, and fixed positioning. A top guard is written first and identical bottom guard last:

```text
TQ1 <first 4 transfer bytes, 8 hex> <run, 16 hex> <seq, 16 hex> <CRC, 8 hex>
```

tmux reads the current active grid with `capture-pane -p -e -N`, without `-a` or `-J`. The adapter reconstructs cells using SGR attributes; top and bottom guards must agree and must match the QR frame. Video uses image detection and frame CRC, without depending on OCR of the guards.

Before manifest completion, receiver caches at most 4MiB and 256 group profiles. The first fully verified acceptable manifest selects the transfer; other transfers are ignored afterwards. Explicit transfer selection is persisted. Live capture drops missed observations naturally and does not queue old screenshots; video processes all decoded frames sequentially.

## Persistence and output

SQLite schema version 1, WAL, `synchronous=FULL`; single writer protected by an OS file lock. Per-shard payload CRC protects persisted incomplete data. Recovered bytes are written at their transport offset and synced before the completion transaction. Resume rehashes completed ranges and checks cached shards before use. No progress-only checkpoint is accepted.

Finalization checks the complete transport length/hash, streams optional zstd decompression with an `original_bytes+1` bound, then checks original length/hash. A temporary file in the chosen output directory is synced and published without clobbering an existing file. Existing identical content may be accepted after hashing; differing content and symlinks are rejected. Archive extraction and execution are not automatic.

## Prepared package format

```text
8-byte ASCII "TQTX0001" | manifest_len: u32 | manifest JSON bytes | transport bytes
```

`send` verifies exact package length and transport SHA-256 before playback. Source snapshot and compression are disk-backed; playback caches at most eight FEC generations. Preparing a file that changes in size/mtime during snapshot is rejected; applications needing stronger consistency must snapshot or quiesce their writer.

References: [Nayuki QR generator](https://www.nayuki.io/page/qr-code-generator-library), [ZXing Rust wrapper](https://github.com/zxing-cpp/zxing-cpp/tree/master/wrappers/rust), [RS core](https://github.com/rust-rse/reed-solomon-erasure/blob/v6.0.0/src/core.rs), [tmux 3.4 capture implementation](https://github.com/tmux/tmux/blob/3.4/cmd-capture-pane.c).
