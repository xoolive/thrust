# thrust-wasm

Build and serve locally:

```bash
wasm-pack build crates/thrust-wasm --target web --dev
python -m http.server 8000 -d crates/thrust-wasm
```

Open `http://localhost:8000`.

Minimal browser usage:

```js
import init, { NasrResolver } from "./pkg/thrust_wasm.js";

await init();

const zip = await fetch("/path/to/28DaySubscription_Effective_2026-02-19.zip")
  .then((r) => r.arrayBuffer());

const resolver = new NasrResolver(new Uint8Array(zip));
const airports = await resolver.airports();
console.log(airports.length);
```

Observable-friendly FAA ArcGIS wrapper:

```js
import { createFaaArcgisResolver } from "./faa_arcgis.js";

const faa = await createFaaArcgisResolver({
  onDatasetProgress: ({ datasetId, ratio }) => {
    if (ratio != null) {
      console.log(datasetId, `${(ratio * 100).toFixed(1)}%`);
    }
  }
});

const airports = await faa.airports.data();
const klax = await faa.airports["KLAX"];
const q = await faa.navaids.search("LAX");

console.log(airports.length, klax?.name, q.length);

await faa.resolve({ airway: "J65" });
```

By default this wrapper is lazy: it downloads only datasets needed by the
collection you query first (for example `airports` only fetches the airport
dataset). To fetch everything up front, pass `eager: true`.

To reduce download size in notebooks, pass only selected datasets:

```js
const faa = await createFaaArcgisResolver({
  datasetIds: [
    "e747ab91a11045e8b3f8a3efd093d3b5_0", // airports
    "c9254c171b6741d3a5e494860761443a_0", // navaids
  ],
});
```
