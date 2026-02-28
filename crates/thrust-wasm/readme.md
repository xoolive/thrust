# thrust-wasm

`thrust-wasm` is the WebAssembly binding crate for `traffic-thrust`.
It provides browser/Node-friendly resolvers for FAA and EUROCONTROL data.

## What is exposed

- FAA NASR resolver from zipped cycle files (`NasrResolver`).
- FAA ArcGIS parsing helpers used by the JS adapter layer.
- EUROCONTROL AIXM resolver from folder-like zip payload maps.
- EUROCONTROL DDR resolver from either:
  - folder-like payload maps (`fromDdrFolder`), or
  - direct archive bytes (`fromDdrArchive`).

## Build locally

Build a single web target quickly:

```bash
wasm-pack build crates/thrust-wasm --target web --dev
```

Build publish-ready multi-target npm outputs (esm/web/nodejs):

```bash
cd crates/thrust-wasm
just pkg
```

Serve local assets:

```bash
python -m http.server 8000 -d crates/thrust-wasm
```

## Runtime guidance

- Prefer Node/server-side for full raw datasets (AIXM, DDR, full NASR).
- In browser docs/notebooks, use scoped subsets and lazy loading.
- For DDR folder payloads, expected keys are:
  `navpoints.nnpt`, `routes.routes`, `airports.arp`,
  `sectors.are`, `sectors.sls`, `free_route.are`,
  `free_route.sls`, `free_route.frp`.

## Minimal usage

```js
import init, { NasrResolver } from "./pkg/web/thrust_wasm.js";

await init();

const zip = await fetch("/path/to/28DaySubscription_Effective_2026-02-19.zip")
  .then((r) => r.arrayBuffer());

const resolver = new NasrResolver(new Uint8Array(zip));
const airports = await resolver.airports();
console.log(airports.length);
```

EUROCONTROL DDR from archive bytes:

```js
const ddrZip = await fetch("/path/to/ENV_PostOPS_AIRAC_2111.zip")
  .then((r) => r.arrayBuffer());
const ddr = EurocontrolResolver.fromDdrArchive(new Uint8Array(ddrZip));
console.log(ddr.resolve_airport("EHAM"));
```
