use std::collections::HashMap;

use js_sys::Array;
use serde_json::Value;
use thrust::data::faa::arcgis::parse_arcgis_features;
use wasm_bindgen::prelude::*;

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
pub struct FaaArcgisResolver {
    airports: Vec<AirportRecord>,
    airspaces: Vec<AirspaceRecord>,
    navaids: Vec<NavpointRecord>,
    airways: Vec<AirwayRecord>,
    airport_index: HashMap<String, Vec<usize>>,
    airspace_index: HashMap<String, Vec<usize>>,
    navaid_index: HashMap<String, Vec<usize>>,
    airway_index: HashMap<String, Vec<usize>>,
    sid_index: HashMap<String, Vec<usize>>,
    star_index: HashMap<String, Vec<usize>>,
}

fn procedure_lookup_keys(name: &str) -> Vec<String> {
    let upper = name.trim().to_uppercase();
    if upper.is_empty() {
        return Vec::new();
    }
    let mut out = vec![upper.clone()];
    let compact = upper.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
    if !compact.is_empty() {
        out.push(compact.clone());
        if compact.len() > 4 && compact[compact.len() - 4..].chars().all(|c| c.is_ascii_alphabetic()) {
            out.push(compact[..compact.len() - 4].to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

#[wasm_bindgen]
impl FaaArcgisResolver {
    #[wasm_bindgen(constructor)]
    pub fn new(feature_collections_json: JsValue) -> Result<FaaArcgisResolver, JsValue> {
        let payloads = Array::from(&feature_collections_json);
        let mut features: Vec<Value> = Vec::new();
        for payload in payloads.iter() {
            let value: Value =
                serde_wasm_bindgen::from_value(payload).map_err(|e| JsValue::from_str(&e.to_string()))?;
            let arr = value
                .get("features")
                .and_then(|x| x.as_array())
                .cloned()
                .unwrap_or_default();
            features.extend(arr);
        }

        let dataset = parse_arcgis_features(&features);
        let airports: Vec<AirportRecord> = dataset.airports.into_iter().map(Into::into).collect();
        let airspaces: Vec<AirspaceRecord> = dataset.airspaces.into_iter().map(Into::into).collect();
        let navaids: Vec<NavpointRecord> = dataset.navaids.into_iter().map(Into::into).collect();
        let airways: Vec<AirwayRecord> = dataset.airways.into_iter().map(Into::into).collect();

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

        let mut airspace_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, a) in airspaces.iter().enumerate() {
            airspace_index.entry(a.designator.to_uppercase()).or_default().push(i);
        }

        let mut navaid_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, n) in navaids.iter().enumerate() {
            navaid_index.entry(n.code.clone()).or_default().push(i);
        }

        let mut airway_index: HashMap<String, Vec<usize>> = HashMap::new();
        let mut sid_index: HashMap<String, Vec<usize>> = HashMap::new();
        let mut star_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, a) in airways.iter().enumerate() {
            airway_index.entry(normalize_airway_name(&a.name)).or_default().push(i);
            airway_index.entry(a.name.to_uppercase()).or_default().push(i);
            match a.route_class.as_deref().map(|s| s.to_uppercase()) {
                Some(rc) if rc == "DP" => {
                    for key in procedure_lookup_keys(&a.name) {
                        sid_index.entry(key).or_default().push(i);
                    }
                }
                Some(rc) if rc == "AP" => {
                    for key in procedure_lookup_keys(&a.name) {
                        star_index.entry(key).or_default().push(i);
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            airports,
            airspaces,
            navaids,
            airways,
            airport_index,
            airspace_index,
            navaid_index,
            airway_index,
            sid_index,
            star_index,
        })
    }

    fn resolve_procedure_airway_by_kind(&self, kind: &str, name: &str) -> Option<AirwayRecord> {
        let index = match kind {
            "SID" => &self.sid_index,
            "STAR" => &self.star_index,
            _ => return None,
        };
        for key in procedure_lookup_keys(name) {
            if let Some(i) = index.get(&key).and_then(|idx| idx.first()).copied() {
                if let Some(item) = self.airways.get(i) {
                    return Some(item.clone());
                }
            }
        }
        None
    }

    pub fn airports(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.airports).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn fixes(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.navaids).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn navaids(&self) -> Result<JsValue, JsValue> {
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

    pub fn resolve_fix(&self, code: String) -> Result<JsValue, JsValue> {
        let key = code.to_uppercase();
        // Prefer records with kind == "fix"; fall back to the first match.
        let item = self
            .navaid_index
            .get(&key)
            .and_then(|indices| {
                indices
                    .iter()
                    .filter_map(|&i| self.navaids.get(i))
                    .find(|r| r.kind == "fix")
                    .or_else(|| indices.first().and_then(|&i| self.navaids.get(i)))
            })
            .cloned();

        serde_wasm_bindgen::to_value(&item).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn resolve_navaid(&self, code: String) -> Result<JsValue, JsValue> {
        let key = code.to_uppercase();
        // Prefer records with kind == "navaid"; fall back to the first match.
        let item = self
            .navaid_index
            .get(&key)
            .and_then(|indices| {
                indices
                    .iter()
                    .filter_map(|&i| self.navaids.get(i))
                    .find(|r| r.kind == "navaid")
                    .or_else(|| indices.first().and_then(|&i| self.navaids.get(i)))
            })
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

    pub fn resolve_sid(&self, name: String) -> Result<JsValue, JsValue> {
        let item = self.resolve_procedure_airway_by_kind("SID", &name);
        serde_wasm_bindgen::to_value(&item).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn resolve_star(&self, name: String) -> Result<JsValue, JsValue> {
        let item = self.resolve_procedure_airway_by_kind("STAR", &name);
        serde_wasm_bindgen::to_value(&item).map_err(|e| JsValue::from_str(&e.to_string()))
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

    /// Parse and resolve a raw ICAO field 15 route string into geographic segments.
    ///
    /// Same contract as `EurocontrolResolver::enrichRoute` and `NasrResolver::enrichRoute` —
    /// returns a JS array of `{ start, end, name? }` segment objects resolved against the
    /// FAA ArcGIS nav data.
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
        let mut current_segment_type: Option<String> = None;

        let resolve_code = |code: &str| -> Option<WasmPoint> {
            let key = code.split('/').next().unwrap_or(code).trim().to_uppercase();
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
                        segment_type: Some("route".to_string()),
                        connector: Some(airway_name.to_string()),
                    });
                    prev = next;
                }
                true
            };

        let expand_procedure_from_entry =
            |kind: &str, procedure_name: &str, entry: &WasmPoint, segs: &mut Vec<RouteSegment>| -> Option<WasmPoint> {
                let airway = self.resolve_procedure_airway_by_kind(kind, procedure_name)?;
                let pts = &airway.points;
                if pts.len() < 2 {
                    return None;
                }
                let entry_name = entry.name.as_deref().unwrap_or("").to_uppercase();
                let start_idx = pts.iter().position(|p| p.code.to_uppercase() == entry_name)?;
                if start_idx >= pts.len() - 1 {
                    return None;
                }
                let mut prev = entry.clone();
                for pt in &pts[start_idx + 1..] {
                    let next = WasmPoint {
                        latitude: pt.latitude,
                        longitude: pt.longitude,
                        name: Some(pt.code.clone()),
                        kind: Some(pt.kind.clone()),
                    };
                    segs.push(RouteSegment {
                        start: prev,
                        end: next.clone(),
                        name: Some(procedure_name.to_string()),
                        segment_type: Some(kind.to_string()),
                        connector: Some(procedure_name.to_string()),
                    });
                    prev = next;
                }
                Some(prev)
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
                                    name: Some(airway_name.clone()),
                                    segment_type: Some("unresolved".to_string()),
                                    connector: Some(airway_name),
                                });
                            }
                        } else if let Some(prev) = last_point.take() {
                            let seg_name = current_connector.take();
                            let seg_type = current_segment_type.take();
                            let seg_connector = if seg_type.as_deref() == Some("dct") {
                                Some("DCT".to_string())
                            } else {
                                seg_name.clone()
                            };
                            segments.push(RouteSegment {
                                start: prev,
                                end: exit.clone(),
                                name: seg_name,
                                segment_type: seg_type,
                                connector: seg_connector,
                            });
                        } else {
                            current_connector = None;
                            current_segment_type = None;
                        }
                        last_point = Some(exit);
                    }
                }
                Field15Element::Connector(connector) => match connector {
                    Connector::Airway(name) => {
                        if let Some(entry) = last_point.take() {
                            pending_airway = Some((name.clone(), entry));
                            current_segment_type = None;
                        } else {
                            current_connector = Some(name.clone());
                            current_segment_type = Some("unresolved".to_string());
                        }
                    }
                    Connector::Direct => {
                        current_connector = None;
                        current_segment_type = Some("dct".to_string());
                    }
                    Connector::Sid(name) => {
                        if let Some(entry) = last_point.clone() {
                            if let Some(end) = expand_procedure_from_entry("SID", name, &entry, &mut segments) {
                                last_point = Some(end);
                                current_connector = None;
                                pending_airway = None;
                                current_segment_type = None;
                            } else {
                                current_connector = Some(name.clone());
                                current_segment_type = Some("unresolved".to_string());
                            }
                        } else {
                            current_connector = Some(name.clone());
                            current_segment_type = Some("unresolved".to_string());
                        }
                    }
                    Connector::Star(name) => {
                        if let Some(entry) = last_point.clone() {
                            if let Some(end) = expand_procedure_from_entry("STAR", name, &entry, &mut segments) {
                                last_point = Some(end);
                                current_connector = None;
                                pending_airway = None;
                                current_segment_type = None;
                            } else {
                                current_connector = Some(name.clone());
                                current_segment_type = Some("unresolved".to_string());
                            }
                        } else {
                            current_connector = Some(name.clone());
                            current_segment_type = Some("unresolved".to_string());
                        }
                    }
                    Connector::Nat(name) => {
                        current_connector = Some(name.clone());
                        current_segment_type = Some("NAT".to_string());
                    }
                    Connector::Pts(name) => {
                        current_connector = Some(name.clone());
                        current_segment_type = Some("PTS".to_string());
                    }
                    _ => {}
                },
                Field15Element::Modifier(_) => {}
            }
        }

        serde_wasm_bindgen::to_value(&segments).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{procedure_lookup_keys, FaaArcgisResolver};
    use crate::models::{AirwayPointRecord, AirwayRecord};

    #[test]
    fn procedure_lookup_keys_extracts_base_designator() {
        let keys = procedure_lookup_keys("KEPER9ELFBO");
        assert!(keys.contains(&"KEPER9ELFBO".to_string()));
        assert!(keys.contains(&"KEPER9E".to_string()));
    }

    #[test]
    fn resolve_star_uses_ap_airway_records() {
        let resolver = FaaArcgisResolver {
            airports: Vec::new(),
            airspaces: Vec::new(),
            navaids: Vec::new(),
            airways: vec![AirwayRecord {
                name: "KEPER9ELFBO".to_string(),
                source: "faa_arcgis".to_string(),
                route_class: Some("AP".to_string()),
                points: vec![
                    AirwayPointRecord {
                        code: "KEPER".to_string(),
                        raw_code: "KEPER".to_string(),
                        kind: "fix".to_string(),
                        latitude: 44.0,
                        longitude: 2.0,
                    },
                    AirwayPointRecord {
                        code: "LFBO".to_string(),
                        raw_code: "LFBO".to_string(),
                        kind: "airport".to_string(),
                        latitude: 43.6,
                        longitude: 1.4,
                    },
                ],
            }],
            airport_index: std::collections::HashMap::new(),
            airspace_index: std::collections::HashMap::new(),
            navaid_index: std::collections::HashMap::new(),
            airway_index: std::collections::HashMap::new(),
            sid_index: std::collections::HashMap::new(),
            star_index: {
                let mut m = std::collections::HashMap::new();
                m.insert("KEPER9E".to_string(), vec![0]);
                m
            },
        };

        let star = resolver
            .resolve_procedure_airway_by_kind("STAR", "KEPER9E")
            .expect("missing STAR");
        assert_eq!(star.route_class.as_deref(), Some("AP"));
        assert_eq!(star.name, "KEPER9ELFBO");
    }
}
