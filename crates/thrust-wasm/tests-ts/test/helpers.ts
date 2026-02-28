import { mkdirSync, readFileSync, writeFileSync, existsSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { homedir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";

if ((process.env.http_proxy || process.env.HTTP_PROXY) && !process.env.NODE_USE_ENV_PROXY) {
  process.env.NODE_USE_ENV_PROXY = "1";
}

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = resolve(here, "../../pkg");
const workspaceRoot = resolve(here, "../../../..");

function loadDotEnvFile(): void {
  const envPath = resolve(workspaceRoot, ".env");
  if (!existsSync(envPath)) {
    return;
  }
  const raw = readFileSync(envPath, "utf-8");
  for (const line of raw.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) {
      continue;
    }
    const eq = trimmed.indexOf("=");
    if (eq <= 0) {
      continue;
    }
    const key = trimmed.slice(0, eq).trim();
    const value = trimmed.slice(eq + 1).trim();
    if (!process.env[key]) {
      process.env[key] = value;
    }
  }
}

loadDotEnvFile();

const rawCacheRoot = process.env.FAA_TEST_DATA_DIR ?? join(homedir(), ".cache", "thrust-faa");
const cacheRoot = rawCacheRoot.startsWith("~/")
  ? join(homedir(), rawCacheRoot.slice(2))
  : rawCacheRoot;
const arcgisDir = join(cacheRoot, "arcgis");
const nasrDir = join(cacheRoot, "nasr");

let wasmModulePromise: Promise<any> | null = null;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function requestWithRetry(url: string, init?: RequestInit, attempts = 3): Promise<Response> {
  let lastError: unknown = null;
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      const response = await fetch(url, init);
      if (response.status === 429 || response.status >= 500) {
        if (attempt + 1 < attempts) {
          await sleep(500 * (attempt + 1));
          continue;
        }
      }
      return response;
    } catch (error) {
      lastError = error;
      if (attempt + 1 < attempts) {
        await sleep(500 * (attempt + 1));
        continue;
      }
    }
  }

  throw lastError instanceof Error ? lastError : new Error(`Failed to fetch ${url}`);
}

export function getCachePath(...parts: string[]): string {
  return join(cacheRoot, ...parts);
}

async function fetchBytes(url: string): Promise<Uint8Array> {
  const response = await requestWithRetry(url, { redirect: "follow" });
  if (!response.ok) {
    throw new Error(`Failed to fetch ${url}: ${response.status} ${response.statusText}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

export async function ensureArcgisCacheFile(filename: string, datasetId: string): Promise<string> {
  const filePath = join(arcgisDir, filename);
  if (existsSync(filePath) && readFileSync(filePath).length > 0) {
    return filePath;
  }

  mkdirSync(arcgisDir, { recursive: true });
  const url = `https://opendata.arcgis.com/datasets/${datasetId}.geojson`;
  const body = await fetchBytes(url);
  writeFileSync(filePath, body);
  return filePath;
}

async function firstReachableNasrUrl(): Promise<string> {
  const explicit = process.env.FAA_NASR_URL;
  if (explicit) {
    return explicit;
  }

  const now = new Date();
  const years = [now.getUTCFullYear() + 1, now.getUTCFullYear(), now.getUTCFullYear() - 1];
  const candidates: string[] = [];
  if (process.env.FAA_NASR_AIRAC) {
    candidates.push(process.env.FAA_NASR_AIRAC);
  }

  for (const year of years) {
    const yy = year % 100;
    for (let cycle = 13; cycle >= 1; cycle -= 1) {
      candidates.push(`${yy.toString().padStart(2, "0")}${cycle.toString().padStart(2, "0")}`);
    }
  }

  const uniqueCodes = [...new Set(candidates)];

  const epoch = Date.UTC(1998, 0, 29);
  const msPerDay = 24 * 3600 * 1000;
  const effectiveDateFromAiracCode = (code: string): string => {
    if (!/^\d{4}$/.test(code)) {
      throw new Error(`invalid AIRAC code ${code}`);
    }
    const yy = Number(code.slice(0, 2));
    const cycle = Number(code.slice(2, 4));
    if (cycle < 1 || cycle > 14) {
      throw new Error(`invalid AIRAC cycle ${code}`);
    }

    const year = 2000 + yy;
    const jan1 = Date.UTC(year, 0, 1);
    const extraDays = ((((jan1 - epoch) / msPerDay) % 28) + 28) % 28;
    const yearEpoch = jan1 - (extraDays - 28) * msPerDay;
    const effective = new Date(yearEpoch + (cycle - 1) * 28 * msPerDay);
    const yyyy = effective.getUTCFullYear();
    const mm = `${effective.getUTCMonth() + 1}`.padStart(2, "0");
    const dd = `${effective.getUTCDate()}`.padStart(2, "0");
    return `${yyyy}-${mm}-${dd}`;
  };

  for (const code of uniqueCodes) {
    let date: string;
    try {
      date = effectiveDateFromAiracCode(code);
    } catch {
      continue;
    }
    const url = `https://nfdc.faa.gov/webContent/28DaySub/28DaySubscription_Effective_${date}.zip`;
    const head = await requestWithRetry(url, { method: "HEAD", redirect: "follow" });
    if (head.ok) {
      return url;
    }
    if (head.status === 405 || head.status >= 500 || head.status === 429) {
      const get = await requestWithRetry(url, { redirect: "follow" });
      if (get.ok) {
        return url;
      }
    }
  }

  throw new Error("Could not find a reachable NASR subscription URL");
}

function nasrFilenameFromUrl(url: string): string {
  const clean = url.replace(/\/$/, "");
  const idx = clean.lastIndexOf("/");
  return idx >= 0 ? clean.slice(idx + 1) : clean;
}

function effectiveDateFromAiracCode(code: string): string {
  if (!/^\d{4}$/.test(code)) {
    throw new Error(`invalid AIRAC code ${code}`);
  }
  const yy = Number(code.slice(0, 2));
  const cycle = Number(code.slice(2, 4));
  if (cycle < 1 || cycle > 14) {
    throw new Error(`invalid AIRAC cycle ${code}`);
  }

  const epoch = Date.UTC(1998, 0, 29);
  const msPerDay = 24 * 3600 * 1000;
  const year = 2000 + yy;
  const jan1 = Date.UTC(year, 0, 1);
  const extraDays = ((((jan1 - epoch) / msPerDay) % 28) + 28) % 28;
  const yearEpoch = jan1 - (extraDays - 28) * msPerDay;
  const effective = new Date(yearEpoch + (cycle - 1) * 28 * msPerDay);
  const yyyy = effective.getUTCFullYear();
  const mm = `${effective.getUTCMonth() + 1}`.padStart(2, "0");
  const dd = `${effective.getUTCDate()}`.padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

export async function ensureNasrZipPath(): Promise<string> {
  const explicit = process.env.FAA_NASR_ZIP;
  if (explicit && existsSync(explicit) && readFileSync(explicit).length > 0) {
    return explicit;
  }

  if (existsSync(nasrDir)) {
    const names = readdirSync(nasrDir)
      .filter((name) => name.startsWith("28DaySubscription_Effective_") && name.endsWith(".zip"))
      .sort();
    for (let i = names.length - 1; i >= 0; i -= 1) {
      const candidate = join(nasrDir, names[i]);
      if (existsSync(candidate) && readFileSync(candidate).length > 0) {
        return candidate;
      }
    }
  }

  const configuredAirac = process.env.FAA_NASR_AIRAC;
  if (configuredAirac) {
    try {
      const effective = effectiveDateFromAiracCode(configuredAirac);
      const expected = join(nasrDir, `28DaySubscription_Effective_${effective}.zip`);
      if (existsSync(expected) && readFileSync(expected).length > 0) {
        return expected;
      }
    } catch {
      // fall through
    }
  }

  mkdirSync(nasrDir, { recursive: true });
  const url = await firstReachableNasrUrl();
  const namedPath = join(nasrDir, nasrFilenameFromUrl(url));
  if (existsSync(namedPath) && readFileSync(namedPath).length > 0) {
    return namedPath;
  }
  const body = await fetchBytes(url);
  writeFileSync(namedPath, body);
  return namedPath;
}

export function readJson(path: string): any {
  return JSON.parse(readFileSync(path, "utf-8"));
}

export function readBytes(path: string): Uint8Array {
  return new Uint8Array(readFileSync(path));
}

export async function loadWasmModule(): Promise<any> {
  if (!wasmModulePromise) {
    wasmModulePromise = (async () => {
      const wasmModule = await import(pathToFileURL(resolve(pkgRoot, "thrust_wasm.js")).toString());
      if (typeof wasmModule.default === "function") {
        const wasmBytes = readFileSync(resolve(pkgRoot, "thrust_wasm_bg.wasm"));
        await wasmModule.default({ module_or_path: wasmBytes });
      }
      return wasmModule;
    })();
  }
  return wasmModulePromise;
}

function fromEnvPath(name: string): string | null {
  const value = process.env[name];
  if (!value) {
    return null;
  }
  if (value.startsWith("~/")) {
    return join(homedir(), value.slice(2));
  }
  return value;
}

function firstMatchingFile(dir: string, predicate: (name: string) => boolean): string | null {
  const names = readdirSync(dir);
  for (const name of names) {
    if (predicate(name)) {
      return join(dir, name);
    }
  }
  return null;
}

export type EurocontrolInputs = {
  aixmFolder: Record<string, Uint8Array>;
  ddrFolder: Record<string, string>;
};

export function loadEurocontrolInputs(): EurocontrolInputs | null {
  const aixmRoot = fromEnvPath("THRUST_AIXM_PATH");
  const ddrRoot = fromEnvPath("THRUST_DDR_PATH");
  if (!aixmRoot || !ddrRoot || !existsSync(aixmRoot) || !existsSync(ddrRoot)) {
    return null;
  }

  const airportZipPath = join(aixmRoot, "AirportHeliport.BASELINE.zip");
  const designatedZipPath = join(aixmRoot, "DesignatedPoint.BASELINE.zip");
  const navaidZipPath = join(aixmRoot, "Navaid.BASELINE.zip");
  const routeZipPath = join(aixmRoot, "Route.BASELINE.zip");
  const routeSegmentZipPath = join(aixmRoot, "RouteSegment.BASELINE.zip");
  const arrivalLegZipPath = join(aixmRoot, "ArrivalLeg.BASELINE.zip");
  const departureLegZipPath = join(aixmRoot, "DepartureLeg.BASELINE.zip");
  const siaZipPath = join(aixmRoot, "StandardInstrumentArrival.BASELINE.zip");
  const sidZipPath = join(aixmRoot, "StandardInstrumentDeparture.BASELINE.zip");
  const airspaceZipPath = join(aixmRoot, "Airspace.BASELINE.zip");
  if (
    !existsSync(airportZipPath) ||
    !existsSync(designatedZipPath) ||
    !existsSync(navaidZipPath) ||
    !existsSync(routeZipPath) ||
    !existsSync(routeSegmentZipPath) ||
    !existsSync(arrivalLegZipPath) ||
    !existsSync(departureLegZipPath) ||
    !existsSync(siaZipPath) ||
    !existsSync(sidZipPath) ||
    !existsSync(airspaceZipPath)
  ) {
    return null;
  }

  const navpointsPath = firstMatchingFile(ddrRoot, (name) => name.startsWith("AIRAC_") && name.endsWith(".nnpt"));
  const routesPath = firstMatchingFile(ddrRoot, (name) => name.startsWith("AIRAC_") && name.endsWith(".routes"));
  const airportsPath = firstMatchingFile(ddrRoot, (name) => name.startsWith("VST_") && name.endsWith("_Airports.arp"));
  const sectorsArePath = firstMatchingFile(ddrRoot, (name) => name.startsWith("Sectors_") && name.endsWith(".are"));
  const sectorsSlsPath = firstMatchingFile(ddrRoot, (name) => name.startsWith("Sectors_") && name.endsWith(".sls"));
  const freeRouteArePath = firstMatchingFile(ddrRoot, (name) => name.startsWith("Free_Route_") && name.endsWith(".are"));
  const freeRouteSlsPath = firstMatchingFile(ddrRoot, (name) => name.startsWith("Free_Route_") && name.endsWith(".sls"));
  const freeRouteFrpPath = firstMatchingFile(ddrRoot, (name) => name.startsWith("Free_Route_") && name.endsWith(".frp"));
  if (
    !navpointsPath ||
    !routesPath ||
    !airportsPath ||
    !sectorsArePath ||
    !sectorsSlsPath ||
    !freeRouteArePath ||
    !freeRouteSlsPath ||
    !freeRouteFrpPath
  ) {
    return null;
  }

  return {
    aixmFolder: {
      "AirportHeliport.BASELINE.zip": new Uint8Array(readFileSync(airportZipPath)),
      "DesignatedPoint.BASELINE.zip": new Uint8Array(readFileSync(designatedZipPath)),
      "Navaid.BASELINE.zip": new Uint8Array(readFileSync(navaidZipPath)),
      "Route.BASELINE.zip": new Uint8Array(readFileSync(routeZipPath)),
      "RouteSegment.BASELINE.zip": new Uint8Array(readFileSync(routeSegmentZipPath)),
      "ArrivalLeg.BASELINE.zip": new Uint8Array(readFileSync(arrivalLegZipPath)),
      "DepartureLeg.BASELINE.zip": new Uint8Array(readFileSync(departureLegZipPath)),
      "StandardInstrumentArrival.BASELINE.zip": new Uint8Array(readFileSync(siaZipPath)),
      "StandardInstrumentDeparture.BASELINE.zip": new Uint8Array(readFileSync(sidZipPath)),
      "Airspace.BASELINE.zip": new Uint8Array(readFileSync(airspaceZipPath)),
    },
    ddrFolder: {
      "navpoints.nnpt": readFileSync(navpointsPath, "utf-8"),
      "routes.routes": readFileSync(routesPath, "utf-8"),
      "airports.arp": readFileSync(airportsPath, "utf-8"),
      "sectors.are": readFileSync(sectorsArePath, "utf-8"),
      "sectors.sls": readFileSync(sectorsSlsPath, "utf-8"),
      "free_route.are": readFileSync(freeRouteArePath, "utf-8"),
      "free_route.sls": readFileSync(freeRouteSlsPath, "utf-8"),
      "free_route.frp": readFileSync(freeRouteFrpPath, "utf-8"),
    },
  };
}
