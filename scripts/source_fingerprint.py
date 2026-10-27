"""Shared source/test/vector provenance for verification and release packaging."""
import hashlib


def source_fingerprint(root):
    digest = hashlib.sha256()
    paths = [root / "Cargo.toml", root / "Cargo.lock"]
    paths += [root / "scripts/shadow-ink-get"]
    for folder in ["src", "tests"]:
        paths += sorted((root / folder).rglob("*.rs"))
        paths += sorted((root / folder).rglob("*.json"))
        paths += sorted((root / folder).rglob("*.py"))
    for path in paths:
        digest.update(str(path.relative_to(root)).encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()
