# Contributing

Bug reports and pull requests are welcome. Include your OS, shadow-ink version, backend, terminal/tmux geometry, the exact command, and a small reproducible example. Remove private file paths and content from logs before sharing.

Use Rust 1.88+, CMake, a C/C++ compiler, Python 3.9+, and tmux. FFmpeg is needed for video acceptance tests.

```sh
cargo fmt --check
cargo test --locked
cargo test --locked --no-default-features
cargo clippy --locked --all-targets --all-features -- -D warnings
PATH="$PWD/target/debug:$PATH" python3 -m unittest discover -s tests -p 'test_*.py' -v
```

Changes to the wire format must preserve the documented compatibility guarantees and test vectors. Keep completed, independently hash-verified measurements separate from configuration values or illustrative demo timings. Never commit user data, transfer caches, recordings, credentials, or locally supplied demo images. See [the demo guide](demo/README.md) for the HTML film and reproducible recording workflow.

All contributions are provided under the repository's MIT license. Third-party assets retain their own licenses.

The release and license badges are repository-owned SVGs in `docs/assets/badges/`, so the READMEs do not depend on an external badge service. When publishing a release, update `release.svg` and the release badge alt text in both READMEs to match the published version. The CI badge stays live on GitHub Actions.
