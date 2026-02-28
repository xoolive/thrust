import {
  ensureArcgisCacheFile,
  loadWasmModule,
  readJson,
} from "./helpers";

const airportsPath = await ensureArcgisCacheFile("faa_airports.json");
const atsRoutesPath = await ensureArcgisCacheFile("faa_ats_routes.json");
const designatedPath = await ensureArcgisCacheFile("faa_designated_points.json");
const navaidsPath = await ensureArcgisCacheFile("faa_navaid_components.json");

const collections = [
  readJson(airportsPath),
  readJson(atsRoutesPath),
  readJson(designatedPath),
  readJson(navaidsPath)
];

const wasmModule = await loadWasmModule();
const resolver = new wasmModule.FaaArcgisResolver(collections);

const airports = resolver.airports() as Array<{
  code: string;
  icao?: string | null;
  latitude: number;
  longitude: number;
}>;
if (airports.length < 1000) {
  throw new Error("Expected substantial FAA airport dataset from cache");
}

const airportCodes = new Set(airports.map((a) => String(a.code || "").toUpperCase()));
const airportIcaos = new Set(airports.map((a) => String(a.icao || "").toUpperCase()));
for (const code of ["KLAX", "KATL", "KJFK", "KORD", "CYVR", "CYUL"]) {
  if (!airportCodes.has(code) && !airportIcaos.has(code)) {
    throw new Error(`Missing FAA ArcGIS airport ${code}`);
  }
}

const lax = resolver.resolve_airport("LAX") as { code?: string; latitude?: number } | null;
if (!lax || lax.code !== "LAX" || !(Number(lax.latitude) > 0)) {
  throw new Error("Failed to resolve LAX airport from cached FAA ArcGIS data");
}
const laxName = String((lax as { name?: string } | null)?.name || "").toUpperCase();
if (!laxName.includes("LOS ANGELES")) {
  throw new Error(`Unexpected LAX airport name: ${laxName}`);
}

const basye = resolver.resolve_fix("BASYE") as { code?: string; latitude?: number } | null;
if (!basye || basye.code !== "BASYE") {
  throw new Error("Failed to resolve BASYE fix from cached FAA ArcGIS data");
}

const baf = resolver.resolve_navaid("BAF") as { code?: string; point_type?: string } | null;
if (!baf || baf.code !== "BAF") {
  throw new Error("Failed to resolve BAF navaid from cached FAA ArcGIS data");
}
const bafName = String((baf as { name?: string } | null)?.name || "").toUpperCase();
if (!bafName.includes("BARNES")) {
  throw new Error(`Unexpected BAF navaid name: ${bafName}`);
}
const pointType = String(baf.point_type || "").toUpperCase();
if (!(pointType.includes("VOR") || pointType.includes("DME") || pointType.includes("TAC"))) {
  throw new Error(`Unexpected BAF point_type: ${pointType}`);
}

const j48 = resolver.resolve_airway("J48") as { name?: string } | null;
if (!j48 || String(j48.name || "").toUpperCase() !== "J48") {
  throw new Error("Failed to resolve J48 airway from cached FAA ArcGIS data");
}

console.log("faa_arcgis.spec.ts passed");
