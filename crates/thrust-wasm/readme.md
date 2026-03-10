# thrust-wasm: WASM Bindings for Aviation Data Parsing

`thrust-wasm` is a WebAssembly binding for the Rust `traffic-thrust` library, providing high-performance aviation data parsing and resolution in browser and Node.js environments.

## Overview

This package exposes efficient resolvers for FAA and EUROCONTROL aviation data, enabling:

- **ICAO Field 15 parsing** — Parse flight plan routes and procedures
- **FAA NASR data** — Query airport, navaid, and airway definitions
- **FAA ArcGIS helpers** — Access geo-tagged aviation facilities
- **EUROCONTROL AIXM data** — Parse detailed airspace and procedure definitions
- **EUROCONTROL DDR data** — Query route networks and designations

## Architecture

The WASM binding exposes high-level resolver classes that abstract away the complexity of data parsing:

```
┌─────────────────────────────────────────┐
│   TypeScript/JavaScript Application     │
│   (Browser or Node.js)                  │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│  WASM Resolver Classes (JS-friendly)    │
│  - NasrResolver                         │
│  - EurocontrolResolver                  │
│  - FaaArcgisResolver                    │
│  - Field15Parser                        │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│  Rust Core Library (traffic-thrust)     │
│  - AIRAC cycle management               │
│  - Field15 tokenization & parsing       │
│  - NASR CSV parsing                     │
│  - AIXM XML parsing                     │
│  - DDR data file parsing                │
└────────────────────────────────────────┘
```

## Quick Start

### FAA NASR Data Resolution

```typescript
import init, { NasrResolver } from "@anomalyco/thrust-wasm";

await init();

const nasrZip = await fetch("/data/28DaySubscription_Effective_2026-02-19.zip")
  .then((r) => r.arrayBuffer());

const resolver = new NasrResolver(new Uint8Array(nasrZip));

// Query airports
const airports = await resolver.airports();
console.log(`Found ${airports.length} airports`);

// Get LAX (Los Angeles International) airport
const lax = await resolver.resolve_airport("KLAX");
console.log(lax.name); // "Los Angeles International"
console.log(lax.latitude, lax.longitude); // 33.9425, -118.4081

// Get JFK (New York) airport
const jfk = await resolver.resolve_airport("KJFK");
console.log(jfk.name); // "John F Kennedy International"

// Resolve a navaid - BAF (Barnes VOR/DME) in Georgia
const baf = await resolver.resolve_navaid("BAF");
console.log(baf.code); // "BAF"
console.log(baf.point_type); // "VOR/DME"

// Resolve a waypoint/fix
const basye = await resolver.resolve_fix("BASYE");
console.log(basye.code); // "BASYE"

// Query an airway
const j48 = await resolver.resolve_airway("J48");
console.log(j48.name); // "J48"
```

### EUROCONTROL DDR Data

```typescript
import init, { EurocontrolResolver } from "@anomalyco/thrust-wasm";

await init();

const ddrZip = await fetch("/data/ENV_PostOPS_AIRAC_2111.zip")
  .then((r) => r.arrayBuffer());

const ddr = EurocontrolResolver.fromDdrArchive(new Uint8Array(ddrZip));
console.log(ddr.resolve_airport("EHAM"));
```

### ICAO Field 15 Parsing

```typescript
import init, { parse_field15 } from "@anomalyco/thrust-wasm";

await init();

// Real-world transatlantic route from Europe to North America
const route = "N0490F360 ELCOB6B ELCOB UT300 SENLO UN502 JSY DCT LIZAD DCT MOPAT DCT LUNIG DCT MOMIN DCT PIKIL/M084F380 NATD HOIST/N0490F380 N756C ANATI/N0441F340 DCT MIVAX DCT OBTEK DCT XORLO ROCKT2";
const elements = parse_field15(route);

// Results in structured elements:
// [
//   { speed: { kts: 490 }, altitude: { FL: 360 } },       // Initial cruise
//   { SID: "ELCOB6B" },                                     // Departure procedure at ELCOB
//   { waypoint: "ELCOB" },
//   { airway: "UT300" },                                    // Upper T-route
//   { waypoint: "SENLO" },                                  // Entry to Nat Track
//   { airway: "UN502" },                                    // Upper N-route
//   { waypoint: "JSY" },
//   { direct_routing: "DCT" },                              // Direct routing
//   { waypoint: "LIZAD" },
//   { altitude_change: { FL: 380 } },                       // Altitude change mid-route
//   { nat_routing: "NATD" },                                // NAT designation
//   { waypoint: "HOIST" },
//   { speed_altitude: { kts: 490, FL: 380 } },              // Speed/altitude constraint
//   { waypoint: "N756C" },
//   // ... more elements
// ]
```

## Installation

```bash
npm install @anomalyco/thrust-wasm
```

## API Reference

### NasrResolver

FAA NASR (National Airspace System Resource) data resolver.

**Methods:**
- `airports(): Promise<Airport[]>` — Get all airports
- `airport(icao: string): Promise<Airport | undefined>` — Query specific airport
- `airways(): Promise<Airway[]>` — Get all airways
- `navaids(): Promise<Navaid[]>` — Get all navaids
- `designated_points(): Promise<DesignatedPoint[]>` — Get waypoints
- `airac_cycle(): string` — Get AIRAC cycle information

### EurocontrolResolver

EUROCONTROL AIXM and DDR data resolver.

**Static Methods:**
- `fromDdrArchive(zipData: Uint8Array): EurocontrolResolver` — Create from DDR archive
- `fromDdrFolder(payload: Record<string, Uint8Array>): EurocontrolResolver` — Create from DDR folder payload

**Instance Methods:**
- `parse_airports(zipData: Uint8Array): void` — Parse AIXM airports
- `parse_navaids(zipData: Uint8Array): void` — Parse AIXM navaids
- `parse_airways(zipData: Uint8Array): void` — Parse AIXM airways
- `resolve_airport(icao: string): Airport | undefined` — Query airport
- `resolve_navpoint(name: string): NavPoint | undefined` — Query navpoint
- `resolve_nat_routes(): NatTrack[]` — Get NAT track information

### parse_field15(route: string): Field15Element[]

Parse ICAO Field 15 route string into structured elements.

## Design Patterns

**Browser vs. Server:**
- Browser: Use pre-subset data to minimize bundle size
- Server: Use full datasets for complete coverage

**DDR Folder Payloads:**

When using `fromDdrFolder`, expected keys are:
```typescript
{
  "navpoints.nnpt": Uint8Array,
  "routes.routes": Uint8Array,
  "airports.arp": Uint8Array,
  "sectors.are": Uint8Array,
  "sectors.sls": Uint8Array,
  "free_route.are": Uint8Array,
  "free_route.sls": Uint8Array,
  "free_route.frp": Uint8Array,
}
```

**Error Handling:**

Most operations throw `JsError` on invalid input:
```typescript
try {
  const data = await resolver.airports();
} catch (error) {
  console.error("Error:", error.message);
}
```

## Build Locally

### Prerequisites
- Rust 1.70+
- Node.js 18+
- `wasm-pack` (`cargo install wasm-pack`)

### Build for Web
```bash
wasm-pack build crates/thrust-wasm --target web --dev
```

### Build Multi-Target (ESM/Web/Node)
```bash
cd crates/thrust-wasm
just pkg
```

### Serve Local Package
```bash
python -m http.server 8000 -d crates/thrust-wasm/pkg
```

### Run Tests
```bash
cd crates/thrust-wasm/tests-ts
npm test
```

## Data Format Reference

**NASR Subscription Files:**
Download from [FAA NASR](https://www.faa.gov/air_traffic/publications/notices_and_procedures/notices/search/).

**EUROCONTROL AIXM:**
Available from [EUROCONTROL B2B](https://www.eurocontrol.int/service-portfolio/aeronautical-information-exchange-model).
- AirportHeliport.BASELINE.zip
- Navaid.BASELINE.zip
- Route.BASELINE.zip
- DesignatedPoint.BASELINE.zip
- RouteSegment.BASELINE.zip
- StandardInstrumentDeparture.BASELINE.zip
- StandardInstrumentArrival.BASELINE.zip

**EUROCONTROL DDR:**
Available from EUROCONTROL B2B (PostOPS database).

## Performance Notes

- **Parsing:** WASM is 5-10× faster than JavaScript for large datasets
- **Memory:** WASM data structures are more compact
- **Startup:** Initial module load takes 100-300ms

## License & Attribution

- **FAA NASR:** Public domain (U.S. government data)
- **EUROCONTROL AIXM/DDR:** Requires license agreement with EUROCONTROL
- **Field 15 parser:** Adapted from [ICAO-F15-Parser](https://github.com/pventon/ICAO-F15-Parser/) (Apache 2.0)

## Contributing

Issues and PRs welcome on [GitHub](https://github.com/anomalyco/thrust).

## License

Apache License 2.0 — See LICENSE file in this directory.
