use std::collections::HashMap;

use wasm_bindgen::prelude::*;

use thrust::data::faa::nasr::parse_resolver_data_from_nasr_bytes;

use crate::models::{
    normalize_airway_name, AirportRecord, AirspaceCompositeRecord, AirspaceLayerRecord, AirspaceRecord, AirwayRecord,
    NavpointRecord, ProcedureRecord,
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
    procedures: Vec<ProcedureRecord>,
    airspaces: Vec<AirspaceRecord>,
    airport_index: HashMap<String, Vec<usize>>,
    navaid_index: HashMap<String, Vec<usize>>,
    airway_index: HashMap<String, Vec<usize>>,
    sid_index: HashMap<String, Vec<usize>>,
    star_index: HashMap<String, Vec<usize>>,
    airspace_index: HashMap<String, Vec<usize>>,
}

#[wasm_bindgen]
impl NasrResolver {
    fn resolve_procedure_by_kind(&self, kind: &str, name: &str) -> Option<ProcedureRecord> {
        let key = name.to_uppercase();
        let idx = match kind {
            "SID" => self.sid_index.get(&key).and_then(|v| v.first()).copied(),
            "STAR" => self.star_index.get(&key).and_then(|v| v.first()).copied(),
            _ => None,
        }?;
        self.procedures.get(idx).cloned()
    }

    fn enrich_route_segments_internal(&self, route: &str) -> Vec<crate::field15::RouteSegment> {
        use crate::field15::ResolvedPoint as WasmPoint;
        use crate::field15::RouteSegment;
        use thrust::data::field15::{Connector, Field15Element, Field15Parser, Point};

        let elements = Field15Parser::parse(route);
        let mut segments: Vec<RouteSegment> = Vec::new();
        let mut last_point: Option<WasmPoint> = None;
        let mut pending_airway: Option<(String, WasmPoint)> = None;
        let mut pending_procedure: Option<String> = None;
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

        let resolve_procedure = |kind: &str, name: &str| -> Option<&ProcedureRecord> {
            let key = name.to_uppercase();
            let idx = match kind {
                "SID" => self.sid_index.get(&key).and_then(|v| v.first()).copied(),
                "STAR" => self.star_index.get(&key).and_then(|v| v.first()).copied(),
                _ => None,
            }?;
            self.procedures.get(idx)
        };

        let expand_procedure_from_entry =
            |procedure_name: &str, kind: &str, entry: &WasmPoint, segs: &mut Vec<RouteSegment>| -> Option<WasmPoint> {
                let proc = resolve_procedure(kind, procedure_name)?;
                let pts = &proc.points;
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
                        } else if let Some(procedure_name) = pending_procedure.take() {
                            if let Some(prev) = last_point.take() {
                                segments.push(RouteSegment {
                                    start: prev,
                                    end: exit.clone(),
                                    name: Some(procedure_name.clone()),
                                    segment_type: Some("unresolved".to_string()),
                                    connector: Some(procedure_name),
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
                        pending_procedure = None;
                        current_segment_type = Some("dct".to_string());
                    }
                    Connector::Sid(name) => {
                        if let Some(entry) = last_point.clone() {
                            if let Some(end) = expand_procedure_from_entry(name, "SID", &entry, &mut segments) {
                                last_point = Some(end);
                                current_connector = None;
                                pending_airway = None;
                                pending_procedure = None;
                                current_segment_type = None;
                            } else {
                                current_connector = Some(name.clone());
                                pending_procedure = Some(name.clone());
                                current_segment_type = Some("unresolved".to_string());
                            }
                        } else {
                            current_connector = Some(name.clone());
                            pending_procedure = Some(name.clone());
                            current_segment_type = Some("unresolved".to_string());
                        }
                    }
                    Connector::Star(name) => {
                        if let Some(entry) = last_point.clone() {
                            if let Some(end) = expand_procedure_from_entry(name, "STAR", &entry, &mut segments) {
                                last_point = Some(end);
                                current_connector = None;
                                pending_airway = None;
                                pending_procedure = None;
                                current_segment_type = None;
                            } else {
                                current_connector = Some(name.clone());
                                pending_procedure = Some(name.clone());
                                current_segment_type = Some("unresolved".to_string());
                            }
                        } else {
                            current_connector = Some(name.clone());
                            pending_procedure = Some(name.clone());
                            current_segment_type = Some("unresolved".to_string());
                        }
                    }
                    Connector::Nat(name) => {
                        current_connector = Some(name.clone());
                        pending_procedure = None;
                        current_segment_type = Some("NAT".to_string());
                    }
                    Connector::Pts(name) => {
                        current_connector = Some(name.clone());
                        pending_procedure = None;
                        current_segment_type = Some("PTS".to_string());
                    }
                    _ => {}
                },
                Field15Element::Modifier(_) => {}
            }
        }

        segments
    }

    #[wasm_bindgen(constructor)]
    pub fn new(zip_bytes: &[u8]) -> Result<NasrResolver, JsValue> {
        let dataset = parse_resolver_data_from_nasr_bytes(zip_bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let airports: Vec<AirportRecord> = dataset.airports.into_iter().map(Into::into).collect();
        let navaids: Vec<NavpointRecord> = dataset.navaids.into_iter().map(Into::into).collect();
        let airways: Vec<AirwayRecord> = dataset.airways.into_iter().map(Into::into).collect();
        let procedures: Vec<ProcedureRecord> = dataset.procedures.into_iter().map(Into::into).collect();
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

        let mut sid_index: HashMap<String, Vec<usize>> = HashMap::new();
        let mut star_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, p) in procedures.iter().enumerate() {
            let key = p.name.to_uppercase();
            match p.procedure_kind.to_uppercase().as_str() {
                "SID" => sid_index.entry(key).or_default().push(i),
                "STAR" => star_index.entry(key).or_default().push(i),
                _ => {}
            }
        }

        let mut airspace_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, a) in airspaces.iter().enumerate() {
            airspace_index.entry(a.designator.to_uppercase()).or_default().push(i);
        }

        Ok(Self {
            airports,
            navaids,
            airways,
            procedures,
            airspaces,
            airport_index,
            navaid_index,
            airway_index,
            sid_index,
            star_index,
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

    pub fn procedures(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.procedures).map_err(|e| JsValue::from_str(&e.to_string()))
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

    pub fn resolve_sid(&self, name: String) -> Result<JsValue, JsValue> {
        let item = self.resolve_procedure_by_kind("SID", &name);

        serde_wasm_bindgen::to_value(&item).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn resolve_star(&self, name: String) -> Result<JsValue, JsValue> {
        let item = self.resolve_procedure_by_kind("STAR", &name);

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
        let segments = self.enrich_route_segments_internal(&route);
        serde_wasm_bindgen::to_value(&segments).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field15::RouteSegment;
    use crate::models::AirwayPointRecord;

    fn sample_resolver() -> NasrResolver {
        let airports = vec![AirportRecord {
            code: "LFBO".to_string(),
            iata: Some("TLS".to_string()),
            icao: Some("LFBO".to_string()),
            name: Some("Toulouse Blagnac".to_string()),
            latitude: 43.6293,
            longitude: 1.363,
            region: None,
            source: "faa_nasr".to_string(),
        }];

        let navaids = vec![
            NavpointRecord {
                code: "KEPER".to_string(),
                identifier: "KEPER".to_string(),
                kind: "fix".to_string(),
                name: Some("KEPER".to_string()),
                latitude: 44.0,
                longitude: 2.0,
                description: None,
                frequency: None,
                point_type: None,
                region: None,
                source: "faa_nasr".to_string(),
            },
            NavpointRecord {
                code: "NIMER".to_string(),
                identifier: "NIMER".to_string(),
                kind: "fix".to_string(),
                name: Some("NIMER".to_string()),
                latitude: 44.5,
                longitude: 2.2,
                description: None,
                frequency: None,
                point_type: None,
                region: None,
                source: "faa_nasr".to_string(),
            },
        ];

        let procedures = vec![ProcedureRecord {
            name: "KEPER9E".to_string(),
            source: "faa_nasr".to_string(),
            procedure_kind: "STAR".to_string(),
            route_class: Some("AP".to_string()),
            airport: Some("LFBO".to_string()),
            points: vec![
                AirwayPointRecord {
                    code: "KEPER".to_string(),
                    raw_code: "KEPER".to_string(),
                    kind: "fix".to_string(),
                    latitude: 44.0,
                    longitude: 2.0,
                },
                AirwayPointRecord {
                    code: "NIMER".to_string(),
                    raw_code: "NIMER".to_string(),
                    kind: "fix".to_string(),
                    latitude: 44.5,
                    longitude: 2.2,
                },
                AirwayPointRecord {
                    code: "LFBO".to_string(),
                    raw_code: "LFBO".to_string(),
                    kind: "airport".to_string(),
                    latitude: 43.6293,
                    longitude: 1.363,
                },
            ],
        }];

        let mut airport_index = HashMap::new();
        airport_index.insert("LFBO".to_string(), vec![0]);
        airport_index.insert("TLS".to_string(), vec![0]);

        let mut navaid_index = HashMap::new();
        navaid_index.insert("KEPER".to_string(), vec![0]);
        navaid_index.insert("NIMER".to_string(), vec![1]);

        let mut sid_index = HashMap::new();
        let mut star_index = HashMap::new();
        sid_index.insert("FISTO5A".to_string(), vec![]);
        star_index.insert("KEPER9E".to_string(), vec![0]);

        NasrResolver {
            airports,
            navaids,
            airways: Vec::new(),
            procedures,
            airspaces: Vec::new(),
            airport_index,
            navaid_index,
            airway_index: HashMap::new(),
            sid_index,
            star_index,
            airspace_index: HashMap::new(),
        }
    }

    #[test]
    fn resolve_star_returns_notebook_example_procedure() {
        let resolver = sample_resolver();
        let proc = resolver
            .resolve_procedure_by_kind("STAR", "KEPER9E")
            .expect("missing procedure");
        assert_eq!(proc.name, "KEPER9E");
        assert_eq!(proc.procedure_kind, "STAR");
        assert_eq!(proc.route_class.as_deref(), Some("AP"));
        assert_eq!(proc.points.first().map(|p| p.code.as_str()), Some("KEPER"));
    }

    #[test]
    fn enrich_route_expands_terminal_star_segment() {
        let resolver = sample_resolver();
        let segments: Vec<RouteSegment> = resolver.enrich_route_segments_internal("N0430F300 KEPER KEPER9E");

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start.name.as_deref(), Some("KEPER"));
        assert_eq!(segments[0].end.name.as_deref(), Some("NIMER"));
        assert_eq!(segments[0].name.as_deref(), Some("KEPER9E"));
        assert_eq!(segments[0].segment_type.as_deref(), Some("STAR"));
        assert_eq!(segments[0].connector.as_deref(), Some("KEPER9E"));
        assert_eq!(segments[1].start.name.as_deref(), Some("NIMER"));
        assert_eq!(segments[1].end.name.as_deref(), Some("LFBO"));
        assert_eq!(segments[1].name.as_deref(), Some("KEPER9E"));
        assert_eq!(segments[1].segment_type.as_deref(), Some("STAR"));
        assert_eq!(segments[1].connector.as_deref(), Some("KEPER9E"));
    }

    #[test]
    fn enrich_route_nat_connector_is_preserved() {
        let resolver = sample_resolver();
        let segments: Vec<RouteSegment> = resolver.enrich_route_segments_internal("N0430F300 KEPER NATD NIMER");

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start.name.as_deref(), Some("KEPER"));
        assert_eq!(segments[0].end.name.as_deref(), Some("NIMER"));
        assert_eq!(segments[0].name.as_deref(), Some("NATD"));
        assert_eq!(segments[0].segment_type.as_deref(), Some("NAT"));
        assert_eq!(segments[0].connector.as_deref(), Some("NATD"));
    }

    #[test]
    fn enrich_route_keeps_waypoint_with_slash_modifier_between_connectors() {
        let resolver = sample_resolver();
        let segments: Vec<RouteSegment> =
            resolver.enrich_route_segments_internal("N0430F300 KEPER N756C NIMER/N0441F340 DCT LFBO");

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start.name.as_deref(), Some("KEPER"));
        assert_eq!(segments[0].end.name.as_deref(), Some("NIMER"));
        assert_eq!(segments[0].segment_type.as_deref(), Some("unresolved"));
        assert_eq!(segments[0].connector.as_deref(), Some("N756C"));

        assert_eq!(segments[1].start.name.as_deref(), Some("NIMER"));
        assert_eq!(segments[1].end.name.as_deref(), Some("LFBO"));
        assert_eq!(segments[1].segment_type.as_deref(), Some("dct"));
        assert_eq!(segments[1].connector.as_deref(), Some("DCT"));
    }
}
