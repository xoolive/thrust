use std::collections::HashMap;

use js_sys::Array;
use serde_json::Value;
use wasm_bindgen::prelude::*;

use crate::models::{
    normalize_airway_name, AirportRecord, AirspaceCompositeRecord, AirspaceLayerRecord, AirspaceRecord,
    AirwayPointRecord, AirwayRecord, NavpointRecord,
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

fn value_to_f64(v: Option<&Value>) -> Option<f64> {
    v.and_then(|x| x.as_f64().or_else(|| x.as_i64().map(|n| n as f64)))
}

fn parse_coord(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    if let Some(v) = value.as_f64() {
        return Some(v);
    }
    let s = value.as_str()?.trim();
    let hemi = s.chars().last()?;
    let sign = match hemi {
        'N' | 'E' => 1.0,
        'S' | 'W' => -1.0,
        _ => 1.0,
    };
    let core = s.strip_suffix(hemi).unwrap_or(s);
    let parts: Vec<&str> = core.split('-').collect();
    if parts.len() != 3 {
        return core.parse::<f64>().ok();
    }
    let deg = parts[0].parse::<f64>().ok()?;
    let min = parts[1].parse::<f64>().ok()?;
    let sec = parts[2].parse::<f64>().ok()?;
    Some(sign * (deg + min / 60.0 + sec / 3600.0))
}

fn value_to_string(v: Option<&Value>) -> Option<String> {
    v.and_then(|x| x.as_str().map(|s| s.to_string()))
}

fn value_to_i64(v: Option<&Value>) -> Option<i64> {
    v.and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|n| n as i64)))
}

fn geometry_to_polygons(geometry: &Value) -> Vec<Vec<(f64, f64)>> {
    let gtype = geometry.get("type").and_then(|v| v.as_str());
    let coords = geometry.get("coordinates");

    match (gtype, coords) {
        (Some("Polygon"), Some(c)) => c
            .as_array()
            .and_then(|rings| rings.first())
            .and_then(|ring| ring.as_array())
            .map(|ring| {
                ring.iter()
                    .filter_map(|pt| {
                        let arr = pt.as_array()?;
                        if arr.len() < 2 {
                            return None;
                        }
                        let lon = arr[0].as_f64()?;
                        let lat = arr[1].as_f64()?;
                        Some((lon, lat))
                    })
                    .collect::<Vec<_>>()
            })
            .into_iter()
            .collect(),
        (Some("MultiPolygon"), Some(c)) => c
            .as_array()
            .map(|polys| {
                polys
                    .iter()
                    .filter_map(|poly| {
                        let ring = poly.as_array()?.first()?.as_array()?;
                        Some(
                            ring.iter()
                                .filter_map(|pt| {
                                    let arr = pt.as_array()?;
                                    if arr.len() < 2 {
                                        return None;
                                    }
                                    let lon = arr[0].as_f64()?;
                                    let lat = arr[1].as_f64()?;
                                    Some((lon, lat))
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        _ => vec![],
    }
}

fn arcgis_features_to_airspaces(features: &[Value]) -> Vec<AirspaceRecord> {
    let mut out = Vec::new();
    for feature in features {
        let properties = feature.get("properties").unwrap_or(&Value::Null);
        let geometry = feature.get("geometry").unwrap_or(&Value::Null);
        let polygons = geometry_to_polygons(geometry);
        if polygons.is_empty() {
            continue;
        }

        let designator = value_to_string(properties.get("IDENT"))
            .or_else(|| value_to_string(properties.get("NAME")))
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let name = value_to_string(properties.get("NAME"));
        let type_ = value_to_string(properties.get("TYPE_CODE"));
        let lower =
            value_to_f64(properties.get("LOWER_VAL")).map(|v| if (v + 9998.0).abs() < f64::EPSILON { 0.0 } else { v });
        let upper = value_to_f64(properties.get("UPPER_VAL")).map(|v| {
            if (v + 9998.0).abs() < f64::EPSILON {
                f64::INFINITY
            } else {
                v
            }
        });

        for coords in polygons {
            if coords.len() < 3 {
                continue;
            }
            out.push(AirspaceRecord {
                designator: designator.clone(),
                name: name.clone(),
                type_: type_.clone(),
                lower,
                upper,
                coordinates: coords,
                source: "faa_arcgis".to_string(),
            });
        }
    }
    out
}

fn arcgis_features_to_navpoints(features: &[Value]) -> (Vec<NavpointRecord>, Vec<NavpointRecord>) {
    let mut fixes = Vec::new();
    let mut navaid_groups: HashMap<String, NavpointRecord> = HashMap::new();
    let mut navaid_components: HashMap<String, (bool, bool, bool, bool)> = HashMap::new();

    for feature in features {
        let props = feature.get("properties").unwrap_or(&Value::Null);
        let ident = value_to_string(props.get("IDENT")).unwrap_or_default().to_uppercase();
        if ident.is_empty() {
            continue;
        }

        if props.get("NAV_TYPE").is_some() || props.get("FREQUENCY").is_some() {
            let latitude = parse_coord(props.get("LATITUDE")).unwrap_or(0.0);
            let longitude = parse_coord(props.get("LONGITUDE")).unwrap_or(0.0);
            let group_key = value_to_string(props.get("NAVSYS_ID"))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| ident.clone());

            navaid_groups
                .entry(group_key.clone())
                .or_insert_with(|| NavpointRecord {
                    code: ident.clone(),
                    identifier: ident,
                    kind: "navaid".to_string(),
                    name: value_to_string(props.get("NAME")),
                    latitude,
                    longitude,
                    description: value_to_string(props.get("NAME")),
                    frequency: value_to_f64(props.get("FREQUENCY")),
                    point_type: value_to_string(props.get("TYPE_CODE")),
                    region: value_to_string(props.get("US_AREA")),
                    source: "faa_arcgis".to_string(),
                });

            let entry = navaid_components
                .entry(group_key)
                .or_insert((false, false, false, false));
            match value_to_i64(props.get("NAV_TYPE")) {
                Some(1) => entry.0 = true,
                Some(2) => entry.1 = true,
                Some(3) => entry.2 = true,
                Some(4) => entry.3 = true,
                _ => {}
            }
        } else {
            let latitude = parse_coord(props.get("LATITUDE")).unwrap_or(0.0);
            let longitude = parse_coord(props.get("LONGITUDE")).unwrap_or(0.0);
            fixes.push(NavpointRecord {
                code: ident.clone(),
                identifier: ident.clone(),
                kind: "fix".to_string(),
                name: Some(ident),
                latitude,
                longitude,
                description: value_to_string(props.get("REMARKS")),
                frequency: None,
                point_type: value_to_string(props.get("TYPE_CODE")).map(|s| s.to_uppercase()),
                region: value_to_string(props.get("US_AREA")).or_else(|| value_to_string(props.get("STATE"))),
                source: "faa_arcgis".to_string(),
            });
        }
    }

    let mut navaids: Vec<NavpointRecord> = navaid_groups
        .into_iter()
        .map(|(group_key, mut record)| {
            if let Some((has_ndb, has_dme, has_vor, has_tacan)) = navaid_components.get(&group_key).copied() {
                record.point_type = Some(
                    if has_vor && has_tacan {
                        "VORTAC"
                    } else if has_vor && has_dme {
                        "VOR_DME"
                    } else if has_vor {
                        "VOR"
                    } else if has_tacan {
                        "TACAN"
                    } else if has_dme {
                        "DME"
                    } else if has_ndb {
                        "NDB"
                    } else {
                        record.point_type.as_deref().unwrap_or("NAVAID")
                    }
                    .to_string(),
                );
            }
            record
        })
        .collect();
    navaids.sort_by(|a, b| a.code.cmp(&b.code));

    (fixes, navaids)
}

fn arcgis_features_to_airports(features: &[Value]) -> Vec<AirportRecord> {
    let mut airports = Vec::new();

    for feature in features {
        let props = feature.get("properties").unwrap_or(&Value::Null);
        let ident = value_to_string(props.get("IDENT")).unwrap_or_default().to_uppercase();
        let icao = value_to_string(props.get("ICAO_ID")).map(|x| x.to_uppercase());
        if ident.is_empty() && icao.is_none() {
            continue;
        }

        let latitude = parse_coord(props.get("LATITUDE"));
        let longitude = parse_coord(props.get("LONGITUDE"));
        let (latitude, longitude) = match (latitude, longitude) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => continue,
        };

        let code = if ident.is_empty() {
            icao.clone().unwrap_or_default()
        } else {
            ident.clone()
        };
        if code.is_empty() {
            continue;
        }

        airports.push(AirportRecord {
            code,
            iata: if ident.len() == 3 { Some(ident) } else { None },
            icao,
            name: value_to_string(props.get("NAME")),
            latitude,
            longitude,
            region: value_to_string(props.get("STATE")).or_else(|| value_to_string(props.get("US_AREA"))),
            source: "faa_arcgis".to_string(),
        });
    }

    airports
}

fn arcgis_features_to_airways(features: &[Value]) -> Vec<AirwayRecord> {
    let mut grouped: HashMap<String, Vec<AirwayPointRecord>> = HashMap::new();
    let mut point_id_to_ident: HashMap<String, String> = HashMap::new();

    for feature in features {
        let props = feature.get("properties").unwrap_or(&Value::Null);
        let global_id = value_to_string(props.get("GLOBAL_ID")).map(|s| s.to_uppercase());
        let ident = value_to_string(props.get("IDENT")).map(|s| s.to_uppercase());
        if let (Some(gid), Some(idt)) = (global_id, ident) {
            if !gid.is_empty() && !idt.is_empty() {
                point_id_to_ident.entry(gid).or_insert(idt);
            }
        }
    }

    for feature in features {
        let props = feature.get("properties").unwrap_or(&Value::Null);
        let name = value_to_string(props.get("IDENT")).unwrap_or_default().to_uppercase();
        if name.is_empty() {
            continue;
        }

        let geom = feature.get("geometry").unwrap_or(&Value::Null);
        if geom.get("type").and_then(|x| x.as_str()) != Some("LineString") {
            continue;
        }
        let coords = geom
            .get("coordinates")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();

        let start_id = value_to_string(props.get("STARTPT_ID")).map(|s| s.to_uppercase());
        let end_id = value_to_string(props.get("ENDPT_ID")).map(|s| s.to_uppercase());
        let start_code = start_id
            .as_ref()
            .and_then(|id| point_id_to_ident.get(id).cloned())
            .or(start_id.clone());
        let end_code = end_id
            .as_ref()
            .and_then(|id| point_id_to_ident.get(id).cloned())
            .or(end_id.clone());

        let entry = grouped.entry(name).or_default();
        let coord_len = coords.len();
        for (idx, p) in coords.into_iter().enumerate() {
            let arr = match p.as_array() {
                Some(v) if v.len() >= 2 => v,
                _ => continue,
            };
            let lon = arr[0].as_f64().unwrap_or(0.0);
            let lat = arr[1].as_f64().unwrap_or(0.0);
            if entry
                .last()
                .map(|x| (x.latitude, x.longitude) == (lat, lon))
                .unwrap_or(false)
            {
                continue;
            }

            let raw_code = if idx == 0 {
                start_code.clone().unwrap_or_default()
            } else if idx + 1 == coord_len {
                end_code.clone().unwrap_or_default()
            } else {
                String::new()
            };
            let code = if raw_code.is_empty() {
                format!("{},{}", lat, lon)
            } else {
                raw_code.clone()
            };

            entry.push(AirwayPointRecord {
                code,
                raw_code,
                kind: "point".to_string(),
                latitude: lat,
                longitude: lon,
            });
        }
    }

    grouped
        .into_iter()
        .map(|(name, points)| AirwayRecord {
            name,
            source: "faa_arcgis".to_string(),
            route_class: None,
            points,
        })
        .collect()
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

        let airports = arcgis_features_to_airports(&features);
        let airspaces = arcgis_features_to_airspaces(&features);
        let (fixes, mut navaids) = arcgis_features_to_navpoints(&features);
        navaids.extend(fixes.iter().cloned());
        navaids.sort_by(|a, b| a.code.cmp(&b.code).then(a.point_type.cmp(&b.point_type)));
        navaids.dedup_by(|a, b| {
            a.code == b.code && a.point_type == b.point_type && a.latitude == b.latitude && a.longitude == b.longitude
        });
        let airways = arcgis_features_to_airways(&features);

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
        for (i, a) in airways.iter().enumerate() {
            airway_index.entry(normalize_airway_name(&a.name)).or_default().push(i);
            airway_index.entry(a.name.to_uppercase()).or_default().push(i);
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
        })
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn arcgis_navpoints_parse_coords_and_types() {
        let features = vec![
            json!({
                "properties": {
                    "IDENT": "BAF",
                    "NAME": "BARNES",
                    "LATITUDE": "42-09-43.053N",
                    "LONGITUDE": "072-42-58.318W",
                    "NAV_TYPE": 3,
                    "FREQUENCY": 113.0,
                    "NAVSYS_ID": "NAV-BAF",
                    "US_AREA": "US"
                }
            }),
            json!({
                "properties": {
                    "IDENT": "BAF",
                    "NAME": "BARNES",
                    "LATITUDE": "42-09-43.053N",
                    "LONGITUDE": "072-42-58.318W",
                    "NAV_TYPE": 4,
                    "FREQUENCY": 113.0,
                    "NAVSYS_ID": "NAV-BAF",
                    "US_AREA": "US"
                }
            }),
            json!({
                "properties": {
                    "IDENT": "BASYE",
                    "LATITUDE": "41-20-37.400N",
                    "LONGITUDE": "073-47-54.990W",
                    "TYPE_CODE": "RPT",
                    "US_AREA": "US"
                }
            }),
        ];

        let (fixes, navaids) = arcgis_features_to_navpoints(&features);

        let basye = fixes.iter().find(|f| f.code == "BASYE").unwrap();
        assert!(basye.latitude.abs() > 1.0);
        assert!(basye.longitude.abs() > 1.0);
        assert_eq!(basye.point_type.as_deref(), Some("RPT"));

        let baf = navaids.iter().find(|n| n.code == "BAF").unwrap();
        assert!(baf.latitude.abs() > 1.0);
        assert!(baf.longitude.abs() > 1.0);
        assert_eq!(baf.point_type.as_deref(), Some("VORTAC"));
    }

    /// Regression test: when a fix and a navaid share the same code (e.g. "BAF" is
    /// both a designated fix and the Barnes VORTAC), resolve_navaid must return the
    /// navaid record (with the proper name) and resolve_fix must return the fix record.
    /// Before the fix, the merged+sorted navaids vec placed the fix record first
    /// (point_type "RPT" < "VORTAC" lexicographically), so resolve_navaid("BAF")
    /// returned name="BAF" instead of name="BARNES MUNICIPAL".
    #[test]
    fn resolve_navaid_prefers_navaid_kind_when_fix_has_same_code() {
        // One navaid feature (VOR+TACAN → VORTAC) and one designated-fix feature
        // sharing the same IDENT "BAF".
        let features = vec![
            // Navaid component 1: VOR
            json!({
                "properties": {
                    "IDENT": "BAF",
                    "NAME": "BARNES MUNICIPAL",
                    "LATITUDE": "42-09-43.053N",
                    "LONGITUDE": "072-42-58.318W",
                    "NAV_TYPE": 3,
                    "FREQUENCY": 113.0,
                    "NAVSYS_ID": "NAV-BAF",
                    "US_AREA": "US"
                }
            }),
            // Navaid component 2: TACAN
            json!({
                "properties": {
                    "IDENT": "BAF",
                    "NAME": "BARNES MUNICIPAL",
                    "LATITUDE": "42-09-43.053N",
                    "LONGITUDE": "072-42-58.318W",
                    "NAV_TYPE": 4,
                    "FREQUENCY": 113.0,
                    "NAVSYS_ID": "NAV-BAF",
                    "US_AREA": "US"
                }
            }),
            // Designated fix (no NAV_TYPE) collocated with the VORTAC
            json!({
                "properties": {
                    "IDENT": "BAF",
                    "LATITUDE": "42-09-43.053N",
                    "LONGITUDE": "072-42-58.318W",
                    "TYPE_CODE": "RPT",
                    "US_AREA": "US"
                }
            }),
        ];

        let (fixes, mut navaids) = arcgis_features_to_navpoints(&features);
        // Replicate the merge done in FaaArcgisResolver::new
        navaids.extend(fixes.iter().cloned());
        navaids.sort_by(|a, b| a.code.cmp(&b.code).then(a.point_type.cmp(&b.point_type)));
        navaids.dedup_by(|a, b| {
            a.code == b.code && a.point_type == b.point_type && a.latitude == b.latitude && a.longitude == b.longitude
        });

        let mut navaid_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, n) in navaids.iter().enumerate() {
            navaid_index.entry(n.code.clone()).or_default().push(i);
        }

        // There should be two "BAF" entries: the fix (kind="fix") and the navaid (kind="navaid")
        let baf_indices = navaid_index.get("BAF").expect("BAF missing from index");
        assert_eq!(baf_indices.len(), 2, "expected fix + navaid for BAF");

        // resolve_navaid: must prefer kind=="navaid" → name should contain "BARNES"
        let navaid_result = baf_indices
            .iter()
            .filter_map(|&i| navaids.get(i))
            .find(|r| r.kind == "navaid")
            .or_else(|| baf_indices.first().and_then(|&i| navaids.get(i)))
            .expect("resolve_navaid returned None");
        assert_eq!(navaid_result.kind, "navaid");
        assert_eq!(navaid_result.point_type.as_deref(), Some("VORTAC"));
        assert!(
            navaid_result
                .name
                .as_deref()
                .unwrap_or("")
                .to_uppercase()
                .contains("BARNES"),
            "expected name to contain BARNES, got {:?}",
            navaid_result.name
        );

        // resolve_fix: must prefer kind=="fix" → name should be the ident "BAF"
        let fix_result = baf_indices
            .iter()
            .filter_map(|&i| navaids.get(i))
            .find(|r| r.kind == "fix")
            .or_else(|| baf_indices.first().and_then(|&i| navaids.get(i)))
            .expect("resolve_fix returned None");
        assert_eq!(fix_result.kind, "fix");
        assert_eq!(fix_result.point_type.as_deref(), Some("RPT"));
    }

    #[test]
    fn arcgis_airways_use_endpoint_identifiers_when_available() {
        let features = vec![
            json!({
                "properties": {
                    "GLOBAL_ID": "START-GID",
                    "IDENT": "LANNA"
                }
            }),
            json!({
                "properties": {
                    "GLOBAL_ID": "END-GID",
                    "IDENT": "MOL"
                }
            }),
            json!({
                "properties": {
                    "IDENT": "J48",
                    "STARTPT_ID": "START-GID",
                    "ENDPT_ID": "END-GID"
                },
                "geometry": {
                    "type": "LineString",
                    "coordinates": [
                        [-75.0, 40.5],
                        [-76.0, 40.0],
                        [-79.1, 37.9]
                    ]
                }
            }),
        ];

        let airways = arcgis_features_to_airways(&features);
        let j48 = airways.iter().find(|a| a.name == "J48").unwrap();
        assert!(!j48.points.is_empty());

        let first = j48.points.first().unwrap();
        let last = j48.points.last().unwrap();

        assert_eq!(first.code, "LANNA");
        assert_eq!(first.raw_code, "LANNA");
        assert_eq!(last.code, "MOL");
        assert_eq!(last.raw_code, "MOL");
    }
}
