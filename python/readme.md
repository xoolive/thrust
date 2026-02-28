# thrust

`thrust` is the Python package exposing Rust-powered parsers and resolvers used
by [`traffic`](https://github.com/xoolive/traffic).

It is installable on its own, but its primary role is to provide optimized
backends for `traffic`.

## What it provides

- Field15 parsing and resolution helpers.
- FAA sources (ArcGIS/NASR adapters).
- EUROCONTROL AIXM and DDR source adapters.
- Time/interval utilities backed by Rust extension functions.

## Install (local editable)

```bash
cd python
uv sync --dev
```

## Run tests

```bash
cd python
uv run pytest tests
```

Some EUROCONTROL tests require local datasets via environment variables:

- `THRUST_AIXM_PATH`
- `THRUST_DDR_PATH`

## Type/lint checks

```bash
cd python
uv run ruff check .
uv run ty check thrust tests thrust/core.pyi
```

## Related projects

- Python orchestration layer: <https://github.com/xoolive/traffic>
- Rust/WASM source repo: <https://github.com/xoolive/thrust>
