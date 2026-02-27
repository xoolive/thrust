import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { pathToFileURL } from "node:url";
import { dirname, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = resolve(here, "../../pkg");

const wasmModule = await import(pathToFileURL(resolve(pkgRoot, "thrust_wasm.js")).toString());
const wasmBytes = readFileSync(resolve(pkgRoot, "thrust_wasm_bg.wasm"));

await wasmModule.default({ module_or_path: wasmBytes });

if (!["debug", "release"].includes(wasmModule.wasm_build_profile())) {
  throw new Error("Unexpected build profile marker");
}

const airacCode = wasmModule.airac_code_from_date("2025-08-15") as string;
if (airacCode.length !== 4) {
  throw new Error("airac_code_from_date returned an invalid code");
}

const effective = wasmModule.effective_date_from_airac_code(airacCode) as string;
const interval = wasmModule.airac_interval(airacCode) as {
  begin: string;
  end: string;
};
if (interval.begin !== effective) {
  throw new Error("airac interval begin mismatch");
}

const beginDt = new Date(`${interval.begin}T00:00:00Z`);
const endDt = new Date(`${interval.end}T00:00:00Z`);
if ((endDt.getTime() - beginDt.getTime()) / (24 * 3600 * 1000) !== 28) {
  throw new Error("airac interval duration is not 28 days");
}

const collections = [
  {
    type: "FeatureCollection",
    features: [
      {
        type: "Feature",
        properties: {
          IDENT: "LAX",
          ICAO_ID: "KLAX",
          LATITUDE: 33.9425,
          LONGITUDE: -118.4081,
          NAME: "LOS ANGELES INTERNATIONAL"
        },
        geometry: null
      }
    ]
  }
];

const resolver = new wasmModule.FaaArcgisResolver(collections);
const airports = resolver.airports() as Array<{ code: string }>;
if (airports.length !== 1 || airports[0].code !== "LAX") {
  throw new Error("FaaArcgisResolver airport parsing failed");
}

const klax = resolver.resolve_airport("KLAX") as { code?: string } | null;
if (!klax || klax.code !== "LAX") {
  throw new Error("FaaArcgisResolver airport resolve failed");
}

const runResult = wasmModule.run();
if (runResult !== undefined) {
  throw new Error("run() should return void on success");
}

console.log("thrust-wasm ts tests passed");
