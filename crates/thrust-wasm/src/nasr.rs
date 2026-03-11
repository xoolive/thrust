use std::collections::HashMap;

use wasm_bindgen::prelude::*;

use thrust::data::faa::nasr::parse_resolver_data_from_nasr_bytes;

use crate::models::{
    normalize_airway_name, AirportRecord, AirspaceCompositeRecord, AirspaceLayerRecord, AirspaceRecord, AirwayRecord,
    NavpointRecord,
};

fn compose_airspace(records: Vec<AirspaceRecord>) -> Option<AirspaceCompositeRecord> {
    let first = records.first()?;
    let designator = first.designator.clone();
    let source = first.source.clone();
    let name = records.iter().find_map(|r| r.name.clone());
    let type_ = records.iter().find_map(|r| r.type_.clone());
    let layers = records
        .into_iter()
        .map(|r| AirspaceLayerRecord {
            lower: r.lower,
            upper: r.upper,
            coordinates: r.coordinates,
        })
        .collect();

    Some(AirspaceCompositeRecord {
        designator,
        name,
        type_,
        layers,
        source,
    })
}

#[wasm_bindgen]
pub struct NasrResolver {
    airports: Vec<AirportRecord>,
    navaids: Vec<NavpointRecord>,
    airways: Vec<AirwayRecord>,
    airspaces: Vec<AirspaceRecord>,
    airport_index: HashMap<String, Vec<usize>>,
    navaid_index: HashMap<String, Vec<usize>>,
    airway_index: HashMap<String, Vec<usize>>,
    airspace_index: HashMap<String, Vec<usize>>,
}

#[wasm_bindgen]
impl NasrResolver {
    #[wasm_bindgen(constructor)]
    pub fn new(zip_bytes: &[u8]) -> Result<NasrResolver, JsValue> {
        let dataset = parse_resolver_data_from_nasr_bytes(zip_bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let airports: Vec<AirportRecord> = dataset.airports.into_iter().map(Into::into).collect();
        let navaids: Vec<NavpointRecord> = dataset.navaids.into_iter().map(Into::into).collect();
        let airways: Vec<AirwayRecord> = dataset.airways.into_iter().map(Into::into).collect();
        let airspaces: Vec<AirspaceRecord> = dataset
            .airspaces
            .into_iter()
            .map(|a| AirspaceRecord {
                designator: a.designator,
                name: a.name,
                type_: a.type_,
                lower: a.lower,
                upper: a.upper,
                coordinates: a.coordinates,
                source: "faa_nasr".to_string(),
            })
            .collect();

        let mut airport_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, a) in airports.iter().enumerate() {
            airport_index.entry(a.code.clone()).or_default().push(i);
            if let Some(v) = &a.iata {
                airport_index.entry(v.clone()).or_default().push(i);
            }
            if let Some(v) = &a.icao {
                airport_index.entry(v.clone()).or_default().push(i);
            }
        }

        let mut navaid_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, n) in navaids.iter().enumerate() {
            navaid_index.entry(n.code.clone()).or_default().push(i);
            navaid_index.entry(n.identifier.clone()).or_default().push(i);
        }

        let mut airway_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, a) in airways.iter().enumerate() {
            airway_index.entry(normalize_airway_name(&a.name)).or_default().push(i);
            airway_index.entry(a.name.to_uppercase()).or_default().push(i);
        }

        let mut airspace_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, a) in airspaces.iter().enumerate() {
            airspace_index.entry(a.designator.to_uppercase()).or_default().push(i);
        }

        Ok(Self {
            airports,
            navaids,
            airways,
            airspaces,
            airport_index,
            navaid_index,
            airway_index,
            airspace_index,
        })
    }

    pub fn airports(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.airports).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn resolve_airport(&self, code: String) -> Result<JsValue, JsValue> {
        let key = code.to_uppercase();
        let item = self
            .airport_index
            .get(&key)
            .and_then(|idx| idx.first().copied())
            .and_then(|i| self.airports.get(i))
            .cloned();

        serde_wasm_bindgen::to_value(&item).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn navaids(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.navaids).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn fixes(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.navaids).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn airways(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.airways).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn airspaces(&self) -> Result<JsValue, JsValue> {
        let mut keys = self.airspace_index.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        let rows = keys
            .into_iter()
            .filter_map(|key| {
                let records = self
                    .airspace_index
                    .get(&key)
                    .into_iter()
                    .flat_map(|indices| indices.iter().copied())
                    .filter_map(|idx| self.airspaces.get(idx).cloned())
                    .collect::<Vec<_>>();
                compose_airspace(records)
            })
            .collect::<Vec<_>>();
        serde_wasm_bindgen::to_value(&rows).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn resolve_navaid(&self, code: String) -> Result<JsValue, JsValue> {
        let key = code.to_uppercase();
        let item = self
            .navaid_index
            .get(&key)
            .and_then(|idx| idx.first().copied())
            .and_then(|i| self.navaids.get(i))
            .cloned();

        serde_wasm_bindgen::to_value(&item).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn resolve_fix(&self, code: String) -> Result<JsValue, JsValue> {
        let key = code.to_uppercase();
        let item = self
            .navaid_index
            .get(&key)
            .and_then(|idx| idx.first().copied())
            .and_then(|i| self.navaids.get(i))
            .cloned();

        serde_wasm_bindgen::to_value(&item).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn resolve_airway(&self, name: String) -> Result<JsValue, JsValue> {
        let key = normalize_airway_name(&name);
        let item = self
            .airway_index
            .get(&key)
            .and_then(|idx| idx.first().copied())
            .and_then(|i| self.airways.get(i))
            .cloned();

        serde_wasm_bindgen::to_value(&item).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn resolve_airspace(&self, designator: String) -> Result<JsValue, JsValue> {
        let key = designator.to_uppercase();
        let records = self
            .airspace_index
            .get(&key)
            .into_iter()
            .flat_map(|indices| indices.iter().copied())
            .filter_map(|idx| self.airspaces.get(idx).cloned())
            .collect::<Vec<_>>();

        serde_wasm_bindgen::to_value(&compose_airspace(records)).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Parse and resolve a raw ICAO field 15 route string into geographic segments.
    ///
    /// Same contract as `EurocontrolResolver::enrichRoute` — returns a JS array of
    /// `{ start, end, name? }` segment objects resolved against the FAA NASR nav data.
    #[wasm_bindgen(js_name = enrichRoute)]
    pub fn enrich_route(&self, route: String) -> Result<JsValue, JsValue> {
        use crate::field15::ResolvedPoint as WasmPoint;
        use crate::field15::RouteSegment;
        use thrust::data::field15::{Connector, Field15Element, Field15Parser, Point};

        let elements = Field15Parser::parse(&route);
        let mut segments: Vec<RouteSegment> = Vec::new();
        let mut last_point: Option<WasmPoint> = None;
        let mut pending_airway: Option<(String, WasmPoint)> = None;
        let mut current_connector: Option<String> = None;

        let resolve_code = |code: &str| -> Option<WasmPoint> {
            let key = code.to_uppercase();
            if let Some(idx) = self.airport_index.get(&key).and_then(|v| v.first()) {
                if let Some(a) = self.airports.get(*idx) {
                    return Some(WasmPoint {
                        latitude: a.latitude,
                        longitude: a.longitude,
                        name: Some(a.code.clone()),
                        kind: Some("airport".to_string()),
                    });
                }
            }
            if let Some(idx) = self.navaid_index.get(&key).and_then(|v| v.first()) {
                if let Some(n) = self.navaids.get(*idx) {
                    return Some(WasmPoint {
                        latitude: n.latitude,
                        longitude: n.longitude,
                        name: Some(n.code.clone()),
                        kind: Some(n.kind.clone()),
                    });
                }
            }
            None
        };

        let expand_airway =
            |airway_name: &str, entry: &WasmPoint, exit: &WasmPoint, segs: &mut Vec<RouteSegment>| -> bool {
                let key = crate::models::normalize_airway_name(airway_name);
                let airway = match self
                    .airway_index
                    .get(&key)
                    .and_then(|v| v.first())
                    .and_then(|i| self.airways.get(*i))
                {
                    Some(a) => a,
                    None => return false,
                };
                let pts = &airway.points;
                let entry_name = entry.name.as_deref().unwrap_or("").to_uppercase();
                let exit_name = exit.name.as_deref().unwrap_or("").to_uppercase();
                let entry_pos = pts.iter().position(|p| p.code.to_uppercase() == entry_name);
                let exit_pos = pts.iter().position(|p| p.code.to_uppercase() == exit_name);
                let (from, to) = match (entry_pos, exit_pos) {
                    (Some(f), Some(t)) => (f, t),
                    _ => return false,
                };
                let slice: Vec<&crate::models::AirwayPointRecord> = if from <= to {
                    pts[from..=to].iter().collect()
                } else {
                    pts[to..=from].iter().rev().collect()
                };
                if slice.len() < 2 {
                    return false;
                }
                let mut prev = entry.clone();
                for pt in &slice[1..] {
                    let next = WasmPoint {
                        latitude: pt.latitude,
                        longitude: pt.longitude,
                        name: Some(pt.code.clone()),
                        kind: Some(pt.kind.clone()),
                    };
                    segs.push(RouteSegment {
                        start: prev,
                        end: next.clone(),
                        name: Some(airway_name.to_string()),
                    });
                    prev = next;
                }
                true
            };

        for element in &elements {
            match element {
                Field15Element::Point(point) => {
                    let resolved = match point {
                        Point::Waypoint(name) | Point::Aerodrome(name) => resolve_code(name),
                        Point::Coordinates((lat, lon)) => Some(WasmPoint {
                            latitude: *lat,
                            longitude: *lon,
                            name: None,
                            kind: Some("coords".to_string()),
                        }),
                        Point::BearingDistance { point, .. } => match point.as_ref() {
                            Point::Waypoint(name) | Point::Aerodrome(name) => resolve_code(name),
                            Point::Coordinates((lat, lon)) => Some(WasmPoint {
                                latitude: *lat,
                                longitude: *lon,
                                name: None,
                                kind: Some("coords".to_string()),
                            }),
                            _ => None,
                        },
                    };
                    if let Some(exit) = resolved {
                        if let Some((airway_name, entry)) = pending_airway.take() {
                            let expanded = expand_airway(&airway_name, &entry, &exit, &mut segments);
                            if !expanded {
                                segments.push(RouteSegment {
                                    start: entry,
                                    end: exit.clone(),
                                    name: Some(airway_name),
                                });
                            }
                        } else if let Some(prev) = last_point.take() {
                            segments.push(RouteSegment {
                                start: prev,
                                end: exit.clone(),
                                name: current_connector.take(),
                            });
                        } else {
                            current_connector = None;
                        }
                        last_point = Some(exit);
                    }
                }
                Field15Element::Connector(connector) => match connector {
                    Connector::Airway(name) => {
                        if let Some(entry) = last_point.take() {
                            pending_airway = Some((name.clone(), entry));
                        } else {
                            current_connector = Some(name.clone());
                        }
                    }
                    Connector::Direct => {
                        current_connector = None;
                    }
                    Connector::Sid(name) | Connector::Star(name) => {
                        current_connector = Some(name.clone());
                    }
                    _ => {}
                },
                Field15Element::Modifier(_) => {}
            }
        }

        serde_wasm_bindgen::to_value(&segments).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
