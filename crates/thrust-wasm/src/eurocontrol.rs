use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};

use thrust::data::eurocontrol::aixm::dataset::parse_aixm_folder_bytes;
use thrust::data::eurocontrol::ddr::airports::parse_airports_bytes;
use thrust::data::eurocontrol::ddr::airspaces::{parse_are_bytes, parse_sls_bytes, DdrSectorLayer};
use thrust::data::eurocontrol::ddr::navpoints::parse_navpoints_bytes;
use thrust::data::eurocontrol::ddr::routes::parse_routes_bytes;
use wasm_bindgen::prelude::*;
use zip::read::ZipArchive;

use crate::models::{
    normalize_airway_name, AirportRecord, AirspaceCompositeRecord, AirspaceLayerRecord, AirspaceRecord,
    AirwayPointRecord, AirwayRecord, NavpointRecord,
};

const DDR_EXPECTED_FILES: [&str; 8] = [
    "navpoints.nnpt",
    "routes.routes",
    "airports.arp",
    "sectors.are",
    "sectors.sls",
    "free_route.are",
    "free_route.sls",
    "free_route.frp",
];

// DDR routes are grouped by airway name, which can merge distinct route variants
// into a single sequence. We split a chain only when a consecutive leg is an
// extreme geodesic jump, using a conservative fixed cutoff. Empirical analysis
// on AIRAC_484E showed >=1000 NM gives near-zero same-name AIXM segment matches
// while still catching obvious merges (for example A10: *PR13 -> SIT).
const DDR_AIRWAY_SPLIT_GAP_NM: f64 = 1_000.0;

fn parse_ddr_navpoints(text: &str) -> Result<Vec<NavpointRecord>, JsValue> {
    let points = parse_navpoints_bytes(text.as_bytes()).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(points
        .into_iter()
        .map(|point| {
            let point_type = point.point_type.to_uppercase();
            let kind = if point_type.contains("FIX") || point_type == "WPT" || point_type == "WP" {
                "fix"
            } else {
                "navaid"
            }
            .to_string();

            NavpointRecord {
                code: point.name.to_uppercase(),
                identifier: point.name.to_uppercase(),
                kind,
                name: point.description.clone(),
                latitude: point.latitude,
                longitude: point.longitude,
                description: point.description,
                frequency: None,
                point_type: Some(point_type),
                region: None,
                source: "eurocontrol_ddr".to_string(),
            }
        })
        .collect())
}

fn parse_ddr_airports(text: &str) -> Result<Vec<AirportRecord>, JsValue> {
    let airports = parse_airports_bytes(text.as_bytes()).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(airports
        .into_iter()
        .map(|airport| AirportRecord {
            code: airport.code.clone(),
            iata: None,
            icao: Some(airport.code),
            name: None,
            latitude: airport.latitude,
            longitude: airport.longitude,
            region: None,
            source: "eurocontrol_ddr".to_string(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{parse_ddr_airports, parse_ddr_airspaces, parse_ddr_airways};

    #[test]
    fn parse_lfbo_coordinates_from_ddr_arp() {
        let airports = parse_ddr_airports("LFBO 2618.100000 82.066667\n").expect("DDR airport parsing failed");
        let lfbo = airports.iter().find(|a| a.code == "LFBO").expect("LFBO not found");

        assert!((lfbo.latitude - 43.635).abs() < 1e-9);
        assert!((lfbo.longitude - 1.3677777833333334).abs() < 1e-9);
    }

    #[test]
    fn split_ddr_airway_on_very_large_gap() {
        let text = [
            "L;A10;AR;999999999999;000000000000;YJQ;SP;1",
            "L;A10;AR;999999999999;000000000000;MITEK;SP;2",
            "L;A10;AR;999999999999;000000000000;*PR13;DBP;3",
            "L;A10;AR;999999999999;000000000000;SIT;SP;4",
            "L;A10;AR;999999999999;000000000000;PAXIS;SP;5",
        ]
        .join("\n");

        let navpoints = [
            "YJQ;FIX;10;10;_",
            "MITEK;FIX;10;11;_",
            "*PR13;DBP;10;12;_",
            "SIT;FIX;55;120;_",
            "PAXIS;FIX;55;121;_",
        ]
        .join("\n");

        let airways = parse_ddr_airways(&text, &navpoints).expect("DDR airway parsing failed");
        assert_eq!(airways.len(), 2);
        assert_eq!(airways[0].name, "A10");
        assert_eq!(airways[1].name, "A10");
        assert_eq!(airways[0].points.len(), 3);
        assert_eq!(airways[1].points.len(), 2);
        assert_eq!(airways[0].points[0].code, "YJQ");
        assert_eq!(airways[0].points[2].code, "*PR13");
        assert_eq!(airways[1].points[0].code, "SIT");
        assert_eq!(airways[1].points[1].code, "PAXIS");
    }

    #[test]
    fn keep_ddr_airway_when_gaps_are_reasonable() {
        let text = [
            "L;UM605;AR;999999999999;000000000000;A;SP;1",
            "L;UM605;AR;999999999999;000000000000;B;SP;2",
            "L;UM605;AR;999999999999;000000000000;C;SP;3",
        ]
        .join("\n");

        let navpoints = ["A;FIX;43.6;1.4;_", "B;FIX;44.0;2.0;_", "C;FIX;44.5;3.0;_"].join("\n");
        let airways = parse_ddr_airways(&text, &navpoints).expect("DDR airway parsing failed");
        assert_eq!(airways.len(), 1);
        assert_eq!(airways[0].name, "UM605");
        assert_eq!(airways[0].points.len(), 3);
    }

    #[test]
    fn parse_ddr_airspaces_from_are_and_sls_text() {
        let mut files = HashMap::new();
        files.insert(
            "sectors.are".to_string(),
            ["3 SEC1_POLY", "0 0", "0 60", "60 60"].join("\n"),
        );
        files.insert("sectors.sls".to_string(), ["SEC1 X SEC1_POLY 100 200"].join("\n"));
        files.insert(
            "free_route.are".to_string(),
            ["3 FRA1_POLY", "120 0", "120 60", "180 60"].join("\n"),
        );
        files.insert("free_route.sls".to_string(), ["FRA1 X FRA1_POLY 245 660"].join("\n"));

        let airspaces = parse_ddr_airspaces(&files).expect("DDR airspace parsing should succeed");
        assert_eq!(airspaces.len(), 2);
        assert_eq!(airspaces[0].designator, "SEC1");
        assert_eq!(airspaces[0].type_.as_deref(), Some("SECTOR"));
        assert_eq!(airspaces[1].designator, "FRA1");
        assert_eq!(airspaces[1].type_.as_deref(), Some("FRA"));
    }

    #[test]
    fn parse_ddr_airspaces_enriches_with_spc_collapsed_designators() {
        let mut files = HashMap::new();
        files.insert("sectors.are".to_string(), ["3 P1", "0 0", "0 60", "60 60"].join("\n"));
        files.insert("sectors.sls".to_string(), ["LFBBN1 X P1 195 295"].join("\n"));
        files.insert(
            "sectors.spc".to_string(),
            ["A;LFBBCTA;BORDEAUX U/ACC;AUA;42;_", "S;LFBBN1;ES"].join("\n"),
        );
        files.insert(
            "free_route.are".to_string(),
            ["3 FRA1_POLY", "120 0", "120 60", "180 60"].join("\n"),
        );
        files.insert("free_route.sls".to_string(), ["FRA1 X FRA1_POLY 245 660"].join("\n"));

        let airspaces = parse_ddr_airspaces(&files).expect("DDR airspace parsing should succeed");
        assert!(airspaces.iter().any(|a| a.designator == "LFBBN1"));
        let collapsed = airspaces
            .iter()
            .find(|a| a.designator == "LFBBCTA")
            .expect("Collapsed LFBBCTA should be present");
        assert_eq!(collapsed.name.as_deref(), Some("BORDEAUX U/ACC"));
        assert_eq!(collapsed.type_.as_deref(), Some("AUA"));
        assert_eq!(collapsed.lower, Some(195.0));
        assert_eq!(collapsed.upper, Some(295.0));
    }
}

fn parse_ddr_airways(routes_text: &str, navpoints_text: &str) -> Result<Vec<AirwayRecord>, JsValue> {
    let navpoints = parse_navpoints_bytes(navpoints_text.as_bytes()).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let route_points =
        parse_routes_bytes(routes_text.as_bytes(), &navpoints).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let mut grouped: HashMap<String, Vec<(i32, AirwayPointRecord, bool)>> = HashMap::new();
    let mut route_class_by_name: HashMap<String, String> = HashMap::new();
    for point in route_points {
        let route = point.route.to_uppercase();
        let route_class = point.route_class.to_uppercase();
        let navaid = point.navaid.to_uppercase();
        let seq = point.seq;
        let (lat, lon, kind, has_coords) = match (point.latitude, point.longitude) {
            (Some(lat), Some(lon)) => {
                let point_type = point.point_type.to_uppercase();
                let kind = if point_type.contains("FIX") || point_type == "WPT" || point_type == "WP" {
                    "fix"
                } else {
                    "navaid"
                }
                .to_string();
                (lat, lon, kind, true)
            }
            _ => (0.0, 0.0, "point".to_string(), false),
        };

        route_class_by_name.entry(route.clone()).or_insert(route_class);

        grouped.entry(route).or_default().push((
            seq,
            AirwayPointRecord {
                code: navaid.clone(),
                raw_code: navaid,
                kind,
                latitude: lat,
                longitude: lon,
            },
            has_coords,
        ));
    }

    let mut out = Vec::new();
    for (name, mut points) in grouped {
        points.sort_by_key(|(seq, _, _)| *seq);
        let deduped = points.into_iter().map(|(_, p, has_coords)| (p, has_coords)).fold(
            Vec::<(AirwayPointRecord, bool)>::new(),
            |mut acc, (p, has_coords)| {
                if acc.last().map(|(x, _)| x.code.as_str()) != Some(p.code.as_str()) {
                    acc.push((p, has_coords));
                }
                acc
            },
        );

        if deduped.is_empty() {
            continue;
        }

        let mut variants: Vec<Vec<AirwayPointRecord>> = vec![vec![deduped[0].0.clone()]];
        for idx in 1..deduped.len() {
            let (prev, prev_has_coords) = &deduped[idx - 1];
            let (point, has_coords) = &deduped[idx];
            let split_here = *prev_has_coords
                && *has_coords
                && great_circle_distance_nm(prev.latitude, prev.longitude, point.latitude, point.longitude)
                    >= DDR_AIRWAY_SPLIT_GAP_NM;

            if split_here {
                variants.push(vec![point.clone()]);
            } else if let Some(current) = variants.last_mut() {
                current.push(point.clone());
            }
        }

        let route_class = route_class_by_name.get(&name).cloned();
        for points in variants.into_iter().filter(|points| points.len() >= 2) {
            out.push(AirwayRecord {
                route_class: route_class.clone(),
                name: name.clone(),
                source: "eurocontrol_ddr".to_string(),
                points,
            });
        }
    }
    Ok(out)
}

fn ddr_layers_to_airspaces(layers: Vec<DdrSectorLayer>, type_name: &str) -> Vec<AirspaceRecord> {
    layers
        .into_iter()
        .map(|layer| AirspaceRecord {
            designator: layer.designator,
            name: Some(layer.polygon_name),
            type_: Some(type_name.to_string()),
            lower: Some(layer.lower),
            upper: Some(layer.upper),
            coordinates: layer.coordinates,
            source: "eurocontrol_ddr".to_string(),
        })
        .collect()
}

fn parse_ddr_collapsed_sectors(text: &str) -> Vec<(String, String, Option<String>, Option<String>)> {
    let mut out = Vec::new();
    let mut current_designator = String::new();
    let mut current_name: Option<String> = None;
    let mut current_type: Option<String> = None;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(';').collect();
        if fields.is_empty() {
            continue;
        }

        match fields[0] {
            "A" if fields.len() >= 4 => {
                current_designator = fields[1].trim().to_uppercase();
                current_name = Some(fields[2].trim().to_string()).filter(|v| !v.is_empty() && v != "_");
                current_type = Some(fields[3].trim().to_string()).filter(|v| !v.is_empty() && v != "_");
            }
            "S" if fields.len() >= 2 && !current_designator.is_empty() => {
                out.push((
                    current_designator.clone(),
                    fields[1].trim().to_uppercase(),
                    current_name.clone(),
                    current_type.clone(),
                ));
            }
            _ => {}
        }
    }

    out
}

fn enrich_sector_airspaces_with_spc(sector_airspaces: &[AirspaceRecord], spc_text: &str) -> Vec<AirspaceRecord> {
    let mappings = parse_ddr_collapsed_sectors(spc_text);
    if mappings.is_empty() {
        return Vec::new();
    }

    let mut by_component: HashMap<String, Vec<&AirspaceRecord>> = HashMap::new();
    for layer in sector_airspaces {
        by_component
            .entry(layer.designator.to_uppercase())
            .or_default()
            .push(layer);
    }

    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (designator, component, name, type_name) in mappings {
        let Some(component_layers) = by_component.get(&component) else {
            continue;
        };

        for layer in component_layers {
            let record = AirspaceRecord {
                designator: designator.clone(),
                name: name.clone().or_else(|| layer.name.clone()),
                type_: type_name.clone().or_else(|| layer.type_.clone()),
                lower: layer.lower,
                upper: layer.upper,
                coordinates: layer.coordinates.clone(),
                source: "eurocontrol_ddr".to_string(),
            };

            let first = record.coordinates.first().copied().unwrap_or((0.0, 0.0));
            let sig = format!(
                "{}|{}|{}|{}|{}|{}|{}|{}",
                record.designator,
                record.name.as_deref().unwrap_or(""),
                record.type_.as_deref().unwrap_or(""),
                record.lower.unwrap_or(-1.0),
                record.upper.unwrap_or(-1.0),
                record.coordinates.len(),
                first.0,
                first.1
            );
            if seen.insert(sig) {
                out.push(record);
            }
        }
    }

    out
}

fn parse_ddr_airspaces(files: &HashMap<String, String>) -> Result<Vec<AirspaceRecord>, JsValue> {
    let sectors_are = files
        .get("sectors.are")
        .ok_or_else(|| JsValue::from_str("missing sectors.are"))?;
    let sectors_sls = files
        .get("sectors.sls")
        .ok_or_else(|| JsValue::from_str("missing sectors.sls"))?;
    let sectors_polygons = parse_are_bytes(sectors_are.as_bytes()).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let sector_layers =
        parse_sls_bytes(sectors_sls.as_bytes(), &sectors_polygons).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let free_route_are = files
        .get("free_route.are")
        .ok_or_else(|| JsValue::from_str("missing free_route.are"))?;
    let free_route_sls = files
        .get("free_route.sls")
        .ok_or_else(|| JsValue::from_str("missing free_route.sls"))?;
    let free_route_polygons =
        parse_are_bytes(free_route_are.as_bytes()).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let free_route_layers = parse_sls_bytes(free_route_sls.as_bytes(), &free_route_polygons)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let sector_airspaces = ddr_layers_to_airspaces(sector_layers, "SECTOR");
    let mut out = sector_airspaces.clone();
    if let Some(sectors_spc) = files.get("sectors.spc") {
        out.extend(enrich_sector_airspaces_with_spc(&sector_airspaces, sectors_spc));
    }
    out.extend(ddr_layers_to_airspaces(free_route_layers, "FRA"));
    Ok(out)
}

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

fn great_circle_distance_nm(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let radius_nm = 3440.065_f64;
    let phi1 = lat1.to_radians();
    let phi2 = lat2.to_radians();
    let dphi = (lat2 - lat1).to_radians();
    let dlambda = (lon2 - lon1).to_radians();
    let a = (dphi / 2.0).sin() * (dphi / 2.0).sin()
        + phi1.cos() * phi2.cos() * (dlambda / 2.0).sin() * (dlambda / 2.0).sin();
    2.0 * radius_nm * a.sqrt().asin()
}

fn file_basename(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

fn find_zip_text_entry(ddr_archive: &[u8], predicate: impl Fn(&str) -> bool) -> Result<String, JsValue> {
    let mut archive = ZipArchive::new(Cursor::new(ddr_archive))
        .map_err(|e| JsValue::from_str(&format!("invalid DDR zip archive: {e}")))?;
    for idx in 0..archive.len() {
        let mut entry = archive
            .by_index(idx)
            .map_err(|e| JsValue::from_str(&format!("unable to read DDR zip entry: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let name = file_basename(entry.name()).to_string();
        if !predicate(&name) {
            continue;
        }
        let mut text = String::new();
        entry
            .read_to_string(&mut text)
            .map_err(|e| JsValue::from_str(&format!("unable to decode DDR entry '{name}' as UTF-8 text: {e}")))?;
        return Ok(text);
    }
    Err(JsValue::from_str("matching DDR file not found in archive"))
}

type DdrEntryMatcher = (&'static str, fn(&str) -> bool);

fn ddr_file_key_and_matchers() -> [DdrEntryMatcher; 8] {
    [
        ("navpoints.nnpt", |name: &str| {
            let lower = name.to_ascii_lowercase();
            lower.starts_with("airac_") && lower.ends_with(".nnpt")
        }),
        ("routes.routes", |name: &str| {
            let lower = name.to_ascii_lowercase();
            lower.starts_with("airac_") && lower.ends_with(".routes")
        }),
        ("airports.arp", |name: &str| {
            let lower = name.to_ascii_lowercase();
            lower.starts_with("vst_") && lower.ends_with("_airports.arp")
        }),
        ("sectors.are", |name: &str| {
            let lower = name.to_ascii_lowercase();
            lower.starts_with("sectors_") && lower.ends_with(".are")
        }),
        ("sectors.sls", |name: &str| {
            let lower = name.to_ascii_lowercase();
            lower.starts_with("sectors_") && lower.ends_with(".sls")
        }),
        ("free_route.are", |name: &str| {
            let lower = name.to_ascii_lowercase();
            lower.starts_with("free_route_") && lower.ends_with(".are")
        }),
        ("free_route.sls", |name: &str| {
            let lower = name.to_ascii_lowercase();
            lower.starts_with("free_route_") && lower.ends_with(".sls")
        }),
        ("free_route.frp", |name: &str| {
            let lower = name.to_ascii_lowercase();
            lower.starts_with("free_route_") && lower.ends_with(".frp")
        }),
    ]
}

fn sectors_spc_matcher(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("sectors_") && lower.ends_with(".spc")
}

fn build_from_ddr_text_files(files: HashMap<String, String>) -> Result<EurocontrolResolver, JsValue> {
    for name in DDR_EXPECTED_FILES {
        if !files.contains_key(name) {
            return Err(JsValue::from_str(&format!(
                "missing DDR file '{name}' in dataset payload"
            )));
        }
    }

    let navaids = parse_ddr_navpoints(
        files
            .get("navpoints.nnpt")
            .ok_or_else(|| JsValue::from_str("missing navpoints.nnpt"))?,
    )?;

    let airports = parse_ddr_airports(
        files
            .get("airports.arp")
            .ok_or_else(|| JsValue::from_str("missing airports.arp"))?,
    )?;
    let airways = parse_ddr_airways(
        files
            .get("routes.routes")
            .ok_or_else(|| JsValue::from_str("missing routes.routes"))?,
        files
            .get("navpoints.nnpt")
            .ok_or_else(|| JsValue::from_str("missing navpoints.nnpt"))?,
    )?;
    let airspaces = parse_ddr_airspaces(&files)?;

    EurocontrolResolver::build(airports, navaids, airways, airspaces)
}

#[wasm_bindgen]
pub struct EurocontrolResolver {
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
impl EurocontrolResolver {
    #[wasm_bindgen(constructor)]
    pub fn new(aixm_folder: JsValue) -> Result<EurocontrolResolver, JsValue> {
        let files: HashMap<String, Vec<u8>> =
            serde_wasm_bindgen::from_value(aixm_folder).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let dataset = parse_aixm_folder_bytes(&files).map_err(|e| JsValue::from_str(&e.to_string()))?;

        Self::build(
            dataset.airports.into_iter().map(Into::into).collect(),
            dataset.navaids.into_iter().map(Into::into).collect(),
            dataset.airways.into_iter().map(Into::into).collect(),
            Vec::new(),
        )
    }

    #[wasm_bindgen(js_name = fromDdrFolder)]
    pub fn from_ddr_folder(ddr_folder: JsValue) -> Result<EurocontrolResolver, JsValue> {
        let files: HashMap<String, String> =
            serde_wasm_bindgen::from_value(ddr_folder).map_err(|e| JsValue::from_str(&e.to_string()))?;
        build_from_ddr_text_files(files)
    }

    #[wasm_bindgen(js_name = fromDdrArchive)]
    pub fn from_ddr_archive(ddr_archive: Vec<u8>) -> Result<EurocontrolResolver, JsValue> {
        let mut files: HashMap<String, String> = HashMap::new();
        for (key, matcher) in ddr_file_key_and_matchers() {
            let text = find_zip_text_entry(&ddr_archive, matcher)
                .map_err(|_| JsValue::from_str(&format!("missing DDR file for key '{key}' in archive payload")))?;
            files.insert(key.to_string(), text);
        }
        if let Ok(text) = find_zip_text_entry(&ddr_archive, sectors_spc_matcher) {
            files.insert("sectors.spc".to_string(), text);
        }
        build_from_ddr_text_files(files)
    }

    fn build(
        airports: Vec<AirportRecord>,
        mut navaids: Vec<NavpointRecord>,
        airways: Vec<AirwayRecord>,
        airspaces: Vec<AirspaceRecord>,
    ) -> Result<EurocontrolResolver, JsValue> {
        let mut seen = HashSet::new();
        navaids.retain(|n| {
            let key = format!(
                "{}|{}|{:.8}|{:.8}",
                n.code,
                n.point_type.as_deref().unwrap_or(""),
                n.latitude,
                n.longitude
            );
            seen.insert(key)
        });

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

        Ok(EurocontrolResolver {
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

    /// Parse and resolve a raw ICAO field 15 route string into a sequence of geographic segments.
    ///
    /// Returns a JS array of `{ start, end, name? }` objects where `start` and `end` are
    /// `{ latitude, longitude, name?, kind? }` resolved geographic points.
    ///
    /// Points are resolved against the resolver's navaid/airport indices. Airways are expanded
    /// to their constituent waypoints. Direct (DCT) segments connect the previous resolved point
    /// to the next one. SID/STAR designators are not expanded (no procedure leg data available).
    #[wasm_bindgen(js_name = enrichRoute)]
    pub fn enrich_route(&self, route: String) -> Result<JsValue, JsValue> {
        use crate::field15::ResolvedPoint as WasmPoint;
        use crate::field15::RouteSegment;
        use thrust::data::field15::{Connector, Field15Element, Field15Parser, Point};

        let elements = Field15Parser::parse(&route);
        let mut segments: Vec<RouteSegment> = Vec::new();
        let mut last_point: Option<WasmPoint> = None;
        // When we encounter an airway connector we store it here together with
        // the entry point so we can slice correctly once the exit point is known.
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

        // Expand a pending airway from `entry` to `exit` using the airway's point list.
        // Finds entry and exit by name, then walks forward or backward between them.
        // Returns true if expansion succeeded; false means the caller should fall back
        // to a direct segment labelled with the airway name.
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

                // Find entry and exit positions by name (case-insensitive).
                let entry_name = entry.name.as_deref().unwrap_or("").to_uppercase();
                let exit_name = exit.name.as_deref().unwrap_or("").to_uppercase();

                let entry_pos = pts.iter().position(|p| p.code.to_uppercase() == entry_name);
                let exit_pos = pts.iter().position(|p| p.code.to_uppercase() == exit_name);

                let (from, to) = match (entry_pos, exit_pos) {
                    (Some(f), Some(t)) => (f, t),
                    _ => return false, // one or both endpoints not in this airway
                };

                // Build the slice going forward or backward.
                let slice: Vec<&crate::models::AirwayPointRecord> = if from <= to {
                    pts[from..=to].iter().collect()
                } else {
                    pts[to..=from].iter().rev().collect()
                };

                if slice.len() < 2 {
                    return false;
                }

                // The first point in the slice IS the entry — use the already-resolved
                // entry WasmPoint so coordinates are consistent with what we already emitted.
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
                            // Try to expand the airway between entry and exit.
                            let expanded = expand_airway(&airway_name, &entry, &exit, &mut segments);
                            if !expanded {
                                // Fall back: single labelled segment entry → exit.
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
                        // Stash the airway name and current last_point as entry.
                        // We need the exit point (next Point token) to slice correctly.
                        if let Some(entry) = last_point.take() {
                            pending_airway = Some((name.clone(), entry));
                        } else {
                            // No entry point yet — treat as a labelled connector.
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
