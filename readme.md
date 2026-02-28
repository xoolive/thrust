# thrust

`thrust` is the Rust acceleration layer behind
[`traffic`](https://github.com/xoolive/traffic), with Python bindings and
WASM bindings for browser/Node use-cases.

## Repository layout

- `crates/thrust`: core Rust crate (`traffic-thrust`) for FAA and EUROCONTROL parsing.
- `python`: Python package (`thrust`) built with maturin (`thrust.core` extension module).
- `crates/thrust-wasm`: WASM crate and npm packaging pipeline.

## Highlights

- FAA support:
  - ArcGIS OpenData datasets (airports, navaids, fixes, ATS routes)
  - NASR field15 extraction from cycle zip files
  - NAT bulletin parsing
- EUROCONTROL support:
  - AIXM baseline datasets
  - DDR datasets from either folder paths or direct zip archives
- Multi-language distribution:
  - PyPI wheels via `.github/workflows/wheels.yml`
  - npm package (`thrust-wasm`) via `.github/workflows/npm.yml`

## Local development

Rust checks:

```bash
cargo fmt --all
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-targets --features traffic-thrust/net
```

Python checks:

```bash
cd python
uv sync --dev
uv run ruff check .
uv run ty check thrust
uv run pytest tests
```

WASM packaging:

```bash
cd crates/thrust-wasm
just pkg
```
