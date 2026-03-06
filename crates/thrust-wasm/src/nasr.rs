use std::collections::HashMap;

use wasm_bindgen::prelude::*;

use thrust::data::faa::nasr::{parse_airspaces_from_nasr_bytes, parse_field15_data_from_nasr_bytes};

use crate::models::{
    normalize_airway_name, normalize_point_code, point_kind, AirportRecord, AirspaceCompositeRecord,
    AirspaceLayerRecord, AirspaceRecord, AirwayPointRecord, AirwayRecord, NavpointRecord,
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
        let data = parse_field15_data_from_nasr_bytes(zip_bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let nasr_airspaces =
            parse_airspaces_from_nasr_bytes(zip_bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let points = data.points;
        let airway_segments = data.airways;

        let airports: Vec<AirportRecord> = points
            .iter()
            .filter(|p| p.kind == "AIRPORT")
            .map(|p| {
                let code = p.identifier.to_uppercase();
                let iata = if code.len() == 3 { Some(code.clone()) } else { None };
                let icao = if code.len() == 4 { Some(code.clone()) } else { None };

                AirportRecord {
                    code,
                    iata,
                    icao,
                    name: p.name.clone(),
                    latitude: p.latitude,
                    longitude: p.longitude,
                    region: p.region.clone(),
                    source: "faa_nasr".to_string(),
                }
            })
            .collect();

        let fixes: Vec<NavpointRecord> = points
            .iter()
            .filter(|p| p.kind == "FIX")
            .map(|p| NavpointRecord {
                code: normalize_point_code(&p.identifier),
                identifier: p.identifier.to_uppercase(),
                kind: "fix".to_string(),
                name: p.name.clone(),
                latitude: p.latitude,
                longitude: p.longitude,
                description: p.description.clone(),
                frequency: p.frequency,
                point_type: p.point_type.clone(),
                region: p.region.clone(),
                source: "faa_nasr".to_string(),
            })
            .collect();

        let mut navaids: Vec<NavpointRecord> = points
            .iter()
            .filter(|p| p.kind == "NAVAID")
            .map(|p| NavpointRecord {
                code: normalize_point_code(&p.identifier),
                identifier: p.identifier.to_uppercase(),
                kind: "navaid".to_string(),
                name: p.name.clone(),
                latitude: p.latitude,
                longitude: p.longitude,
                description: p.description.clone(),
                frequency: p.frequency,
                point_type: p.point_type.clone(),
                region: p.region.clone(),
                source: "faa_nasr".to_string(),
            })
            .collect();

        navaids.extend(fixes.iter().cloned());
        navaids.sort_by(|a, b| a.code.cmp(&b.code).then(a.point_type.cmp(&b.point_type)));
        navaids.dedup_by(|a, b| {
            a.code == b.code && a.point_type == b.point_type && a.latitude == b.latitude && a.longitude == b.longitude
        });

        let mut point_index: HashMap<String, AirwayPointRecord> = HashMap::new();
        for p in &points {
            let normalized = normalize_point_code(&p.identifier);
            let record = AirwayPointRecord {
                code: normalized.clone(),
                raw_code: p.identifier.to_uppercase(),
                kind: point_kind(&p.kind),
                latitude: p.latitude,
                longitude: p.longitude,
            };
            point_index.entry(p.identifier.to_uppercase()).or_insert(record.clone());
            point_index.entry(normalized).or_insert(record);
        }

        let mut grouped: HashMap<String, Vec<AirwayPointRecord>> = HashMap::new();
        for seg in airway_segments {
            let route_name = if seg.airway_id.trim().is_empty() {
                seg.airway_name.clone()
            } else {
                seg.airway_id.clone()
            };
            let entry = grouped.entry(route_name).or_default();

            let from_key = seg.from_point.to_uppercase();
            let to_key = seg.to_point.to_uppercase();
            let from = point_index.get(&from_key).cloned().unwrap_or(AirwayPointRecord {
                code: normalize_point_code(&from_key),
                raw_code: from_key.clone(),
                kind: "point".to_string(),
                latitude: 0.0,
                longitude: 0.0,
            });
            let to = point_index.get(&to_key).cloned().unwrap_or(AirwayPointRecord {
                code: normalize_point_code(&to_key),
                raw_code: to_key.clone(),
                kind: "point".to_string(),
                latitude: 0.0,
                longitude: 0.0,
            });

            if entry.last().map(|x| &x.code) != Some(&from.code) {
                entry.push(from);
            }
            if entry.last().map(|x| &x.code) != Some(&to.code) {
                entry.push(to);
            }
        }

        let airways: Vec<AirwayRecord> = grouped
            .into_iter()
            .map(|(name, points)| AirwayRecord {
                name,
                source: "faa_nasr".to_string(),
                route_class: None,
                points,
            })
            .collect();

        let airspaces: Vec<AirspaceRecord> = nasr_airspaces
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
}
