use serde::Serialize;
use wasm_bindgen::prelude::*;

use thrust::data::field15::{Field15Element, Field15Parser};

/// Parse a raw ICAO field 15 route string into a structured token array.
///
/// Returns a JS array of token objects. Each token is one of:
///   - `{ "waypoint": "LACOU" }` — named waypoint or navaid
///   - `{ "aerodrome": "LFPG" }` — ICAO aerodrome code
///   - `{ "coords": [lat, lon] }` — latitude/longitude coordinates
///   - `{ "point_bearing_distance": { "point": ..., "bearing": 180, "distance": 60 } }`
///   - `{ "airway": "UM184" }` — ATS route
///   - `"DCT"` — direct routing
///   - `{ "SID": "RANUX1A" }` — SID designator
///   - `{ "STAR": "LORNI1A" }` — STAR designator
///   - `{ "speed": ..., "altitude": ... }` — speed/altitude modifier
///   - `"VFR"`, `"IFR"`, `"OAT"`, `"GAT"`, `"IFPSTOP"`, `"IFPSTART"`, `{ "STAY": ... }`, ...
#[wasm_bindgen(js_name = parseField15)]
pub fn parse_field15(route: &str) -> Result<JsValue, JsValue> {
    let elements: Vec<Field15Element> = Field15Parser::parse(route);
    serde_wasm_bindgen::to_value(&elements).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// A resolved geographic point in an enriched route.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedPoint {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// A resolved route segment (start → end with optional airway name).
#[derive(Debug, Clone, Serialize)]
pub struct RouteSegment {
    pub start: ResolvedPoint,
    pub end: ResolvedPoint,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
