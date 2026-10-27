"""Inspect Mach-O slices and independently verify their ad-hoc code-page hashes."""
import hashlib
import struct

ARCHES = {0x01000007: "x86_64", 0x0100000C: "aarch64"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def version(value):
    return f"{value >> 16}.{(value >> 8) & 255}.{value & 255}"


def signature(data, offset, size):
    require(offset + size <= len(data), "code signature extends beyond slice")
    blob = data[offset:offset + size]
    magic, length, count = struct.unpack_from(">III", blob)
    require(magic == 0xFADE0CC0 and length <= len(blob), "invalid signature superblob")
    directories = []
    for i in range(count):
        kind, at = struct.unpack_from(">II", blob, 12 + i * 8)
        if kind != 0 and not 0x1000 <= kind < 0x1005:
            continue
        fields = struct.unpack_from(">9I4B", blob, at)
        magic, length, v, flags, hashes, identifier, special, slots, limit, width, algorithm, _, pages = fields
        require(magic == 0xFADE0C02 and at + length <= len(blob), "invalid code directory")
        require(flags & 2, "expected ad-hoc signature, not an implicit signing identity")
        require(v >= 0x20400, "old code signature format")
        require(algorithm == 2 and width == 32, "expected SHA-256 code signature")
        require(limit == offset and hashes + slots * width <= length, "invalid signed code bounds")
        page = (1 << pages) if pages else limit
        require(slots == (limit + page - 1) // page, "invalid code slot count")
        for n in range(slots):
            want = blob[at + hashes + n * width:at + hashes + (n + 1) * width]
            actual = hashlib.sha256(data[n * page:min((n + 1) * page, limit)]).digest()
            require(want == actual, f"code page {n} hash mismatch")
        identity = blob[at + identifier:at + length].split(b"\0", 1)[0].decode()
        directories.append({"identifier": identity, "code_directory_version": hex(v), "code_pages": slots, "page_hashes_verified": True, "ad_hoc": True})
    require(directories, "missing code directory")
    return directories


def thin(data):
    magic, cpu, subtype, kind, count, command_bytes, flags, _ = struct.unpack_from("<8I", data)
    require(magic == 0xFEEDFACF and cpu in ARCHES and kind == 2, "not a supported 64-bit Mach-O executable")
    require(flags & 1 and flags & 0x200000, "expected NOUNDEFS and PIE")
    require(32 + command_bytes <= len(data), "load commands exceed binary")
    libraries, minimum, sdk, signed = [], None, None, None
    offset = 32
    for _ in range(count):
        command, size = struct.unpack_from("<II", data, offset)
        require(size >= 8 and offset + size <= 32 + command_bytes, "invalid load command")
        if command in {0xC, 0x80000018, 0x8000001F, 0x80000023}:
            name = struct.unpack_from("<I", data, offset + 8)[0]
            library = data[offset + name:offset + size].split(b"\0", 1)[0].decode()
            require(library.startswith(("/usr/lib/", "/System/Library/")), f"non-system dynamic dependency: {library}")
            libraries.append(library)
        elif command == 0x8000001C:
            raise ValueError("unexpected runtime search path")
        elif command == 0x32:
            platform, minimum, sdk = struct.unpack_from("<III", data, offset + 8)
            require(platform == 1 and minimum == 0xB0000, "expected macOS 11.0 deployment target")
        elif command == 0x1D:
            at, length = struct.unpack_from("<II", data, offset + 8)
            signed = signature(data, at, length)
        offset += size
    require(offset == 32 + command_bytes and minimum is not None and signed, "missing version or verified signature")
    return {"architecture": ARCHES[cpu], "minimum_macos": version(minimum), "linker_sdk_version": version(sdk), "dynamic_libraries": libraries, "code_signature": signed, "slice_sha256": hashlib.sha256(data).hexdigest()}


def inspect(path):
    data = path.read_bytes()
    slices = []
    if data[:4] == b"\xca\xfe\xba\xbe":
        count = struct.unpack_from(">I", data, 4)[0]
        require(count == 2, "universal2 must contain exactly two architectures")
        ranges = []
        for i in range(count):
            cpu, subtype, offset, size, alignment = struct.unpack_from(">5I", data, 8 + i * 20)
            require(alignment <= 30 and offset % (1 << alignment) == 0, "invalid fat slice alignment")
            require(offset >= 8 + count * 20 and offset + size <= len(data), "fat slice out of range")
            require(all(offset + size <= a or offset >= b for a, b in ranges), "overlapping fat slices")
            ranges.append((offset, offset + size))
            part = thin(data[offset:offset + size])
            require(part["architecture"] == ARCHES[cpu], "fat and thin architecture disagree")
            slices.append(part)
        require({s["architecture"] for s in slices} == {"aarch64", "x86_64"}, "missing architecture")
    else:
        slices.append(thin(data))
    return {"file": path.name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(), "slices": slices, "native_macos_runtime_verified": False}
