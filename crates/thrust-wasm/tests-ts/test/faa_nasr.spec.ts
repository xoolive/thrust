import { ensureNasrZipPath, loadWasmModule, readBytes } from "./helpers";

const nasrPath = await ensureNasrZipPath();
const zipBytes = readBytes(nasrPath);

const wasmModule = await loadWasmModule();
const resolver = new wasmModule.NasrResolver(zipBytes);

const airports = resolver.airports() as Array<{ code: string; icao?: string | null }>;
if (airports.length < 1000) {
  throw new Error("Expected substantial NASR airport dataset from cache");
}

const airportCodes = new Set(airports.map((a) => String(a.code || "").toUpperCase()));
const airportIcaos = new Set(airports.map((a) => String(a.icao || "").toUpperCase()));
for (const code of ["KLAX", "KATL", "KJFK", "KORD"]) {
  if (!airportCodes.has(code) && !airportIcaos.has(code)) {
    throw new Error(`Missing NASR airport ${code}`);
  }
}

const lax = (resolver.resolve_airport("LAX") ?? resolver.resolve_airport("KLAX")) as
  | { code?: string; latitude?: number; longitude?: number; name?: string }
  | null;
if (!lax || !(Number(lax.latitude) !== 0 && Number(lax.longitude) !== 0)) {
  throw new Error("Failed to resolve LAX/KLAX airport from cached NASR data");
}
if (!String(lax.name || "").toUpperCase().includes("LOS ANGELES")) {
  throw new Error(`Unexpected NASR KLAX name: ${String(lax.name || "")}`);
}

const basyeFix = resolver.resolve_fix("BASYE") as { code?: string } | null;
if (!basyeFix || basyeFix.code !== "BASYE") {
  throw new Error("Failed to resolve BASYE fix from cached NASR data");
}

const bafNavaid = resolver.resolve_navaid("BAF") as { code?: string; point_type?: string; name?: string } | null;
if (!bafNavaid || bafNavaid.code !== "BAF") {
  throw new Error("Failed to resolve BAF navaid from cached NASR data");
}
if (!String(bafNavaid.name || "").toUpperCase().includes("BARNES")) {
  throw new Error(`Unexpected NASR BAF name: ${String(bafNavaid.name || "")}`);
}
const pointType = String(bafNavaid.point_type || "").toUpperCase();
if (!(pointType.includes("VOR") || pointType.includes("DME") || pointType.includes("TAC"))) {
  throw new Error(`Unexpected BAF NASR point_type: ${pointType}`);
}

const j48 = resolver.resolve_airway("J48") as { name?: string } | null;
if (!j48 || String(j48.name || "").toUpperCase() !== "J48") {
  throw new Error("Failed to resolve J48 airway from cached NASR data");
}

console.log("faa_nasr.spec.ts passed");
