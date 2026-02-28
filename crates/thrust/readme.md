# traffic-thrust

`traffic-thrust` is the core Rust crate for navigation-data parsing and
resolution used by the Python and WASM layers of this repository.

## Capabilities

- FAA:
  - ArcGIS OpenData dataset parsing
  - NASR cycle zip parsing (field15-oriented entities)
  - NAT bulletin parsing
- EUROCONTROL:
  - AIXM baseline parsing
  - DDR parsing (directory or zip archive input)

## Feature flags

- `net`: enables network fallback fetch logic (disabled by default)
- `rest`: optional HTTP server examples

## Build and test

```bash
cargo check -p traffic-thrust
cargo clippy -p traffic-thrust --all-features --all-targets -- -D warnings
cargo test -p traffic-thrust --all-targets --features net
```
