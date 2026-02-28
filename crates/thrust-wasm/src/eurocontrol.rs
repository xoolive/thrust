use std::collections::HashMap;
use std::io::{BufReader, Cursor, Read};

use quick_xml::{events::Event, name::QName, Reader};
use wasm_bindgen::prelude::*;
use zip::read::ZipArchive;

use crate::models::{normalize_airway_name, AirportRecord, AirwayPointRecord, AirwayRecord, NavpointRecord};

const AIXM_EXPECTED_FILES: [&str; 10] = [
    "AirportHeliport.BASELINE.zip",
    "Navaid.BASELINE.zip",
    "DesignatedPoint.BASELINE.zip",
    "Route.BASELINE.zip",
    "RouteSegment.BASELINE.zip",
    "ArrivalLeg.BASELINE.zip",
    "DepartureLeg.BASELINE.zip",
    "StandardInstrumentArrival.BASELINE.zip",
    "StandardInstrumentDeparture.BASELINE.zip",
    "Airspace.BASELINE.zip",
];

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

type DynError = Box<dyn std::error::Error>;
type PointRefIndex = HashMap<String, AirwayPointRecord>;
type NavpointsWithRefIndex = (Vec<NavpointRecord>, PointRefIndex);

struct Node<'a> {
    name: QName<'a>,
    attributes: HashMap<String, String>,
}

fn find_node<'a, R: std::io::BufRead>(
    reader: &mut Reader<R>,
    lookup: &[QName<'a>],
    end: Option<QName>,
) -> Result<Node<'a>, Box<dyn std::error::Error>> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let attributes = e
                    .attributes()
                    .flatten()
                    .map(|a| {
                        (
                            String::from_utf8_lossy(a.key.as_ref()).to_string(),
                            String::from_utf8_lossy(a.value.as_ref()).to_string(),
                        )
                    })
                    .collect::<HashMap<_, _>>();
                for elt in lookup {
                    if e.name() == *elt {
                        return Ok(Node { name: *elt, attributes });
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                if let Some(end_tag) = end {
                    if e.name() == end_tag {
                        break;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Box::new(e)),
            _ => {}
        }
        buf.clear();
    }
    Err(Box::new(std::io::Error::other("Node not found")))
}

fn read_text<R: std::io::BufRead>(reader: &mut Reader<R>, end: QName) -> Result<String, Box<dyn std::error::Error>> {
    let mut buf = Vec::new();
    let mut text = String::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(e)) => text.push_str(&e.decode()?),
            Ok(Event::End(e)) if e.name() == end => break,
            Ok(Event::Eof) => break,
            Err(e) => return Err(Box::new(e)),
            _ => {}
        }
        buf.clear();
    }
    Ok(text)
}

fn read_baseline_xml_documents(zip_bytes: &[u8]) -> Result<Vec<String>, DynError> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(cursor)?;
    let mut xmls = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if !file.name().ends_with(".BASELINE") {
            continue;
        }
        let mut xml = String::new();
        file.read_to_string(&mut xml)?;
        if !xml.is_empty() {
            xmls.push(xml);
        }
    }
    Ok(xmls)
}

fn parse_aixm_airports(zip_bytes: &[u8]) -> Result<Vec<AirportRecord>, DynError> {
    let mut out = Vec::new();
    for xml in read_baseline_xml_documents(zip_bytes)? {
        let mut reader = Reader::from_reader(BufReader::new(Cursor::new(xml.into_bytes())));
        while find_node(&mut reader, &[QName(b"aixm:AirportHeliport")], None).is_ok() {
            let mut identifier = String::new();
            let mut icao = String::new();
            let mut iata = None;
            let mut name = String::new();
            let mut latitude = 0.0_f64;
            let mut longitude = 0.0_f64;
            while let Ok(node) = find_node(
                &mut reader,
                &[
                    QName(b"gml:identifier"),
                    QName(b"aixm:locationIndicatorICAO"),
                    QName(b"aixm:designatorIATA"),
                    QName(b"aixm:name"),
                    QName(b"aixm:ElevatedPoint"),
                ],
                Some(QName(b"aixm:AirportHeliport")),
            ) {
                match node.name {
                    QName(b"gml:identifier") => identifier = read_text(&mut reader, node.name)?,
                    QName(b"aixm:locationIndicatorICAO") => icao = read_text(&mut reader, node.name)?,
                    QName(b"aixm:designatorIATA") => iata = Some(read_text(&mut reader, node.name)?),
                    QName(b"aixm:name") => name = read_text(&mut reader, node.name)?,
                    QName(b"aixm:ElevatedPoint") => {
                        while let Ok(pos) = find_node(&mut reader, &[QName(b"gml:pos")], Some(node.name)) {
                            let coords: Vec<f64> = read_text(&mut reader, pos.name)?
                                .split_whitespace()
                                .filter_map(|s| s.parse::<f64>().ok())
                                .collect();
                            if coords.len() == 2 {
                                latitude = coords[0];
                                longitude = coords[1];
                            }
                        }
                    }
                    _ => {}
                }
            }

            if !icao.is_empty() {
                let name_value = if name.is_empty() { None } else { Some(name.clone()) };
                out.push(AirportRecord {
                    code: icao.to_uppercase(),
                    iata: iata.clone().map(|v| v.to_uppercase()),
                    icao: Some(icao.to_uppercase()),
                    name: name_value.clone(),
                    latitude,
                    longitude,
                    region: None,
                    source: "eurocontrol_aixm".to_string(),
                });
                if let Some(iata_code) = iata {
                    out.push(AirportRecord {
                        code: iata_code.to_uppercase(),
                        iata: Some(iata_code.to_uppercase()),
                        icao: Some(icao.to_uppercase()),
                        name: name_value,
                        latitude,
                        longitude,
                        region: None,
                        source: "eurocontrol_aixm".to_string(),
                    });
                }
            } else if !identifier.is_empty() {
                let _ = identifier;
            }
        }
    }
    Ok(out)
}

fn parse_aixm_designated_points(zip_bytes: &[u8]) -> Result<NavpointsWithRefIndex, DynError> {
    let mut out = Vec::new();
    let mut by_id: HashMap<String, AirwayPointRecord> = HashMap::new();
    for xml in read_baseline_xml_documents(zip_bytes)? {
        let mut reader = Reader::from_reader(BufReader::new(Cursor::new(xml.into_bytes())));
        while find_node(&mut reader, &[QName(b"aixm:DesignatedPoint")], None).is_ok() {
            let mut identifier = String::new();
            let mut designator = String::new();
            let mut name = None;
            let mut latitude = 0.0_f64;
            let mut longitude = 0.0_f64;
            let mut point_type = None;
            while let Ok(node) = find_node(
                &mut reader,
                &[
                    QName(b"gml:identifier"),
                    QName(b"aixm:name"),
                    QName(b"aixm:designator"),
                    QName(b"aixm:type"),
                    QName(b"aixm:Point"),
                ],
                Some(QName(b"aixm:DesignatedPoint")),
            ) {
                match node.name {
                    QName(b"gml:identifier") => identifier = read_text(&mut reader, node.name)?,
                    QName(b"aixm:name") => name = Some(read_text(&mut reader, node.name)?),
                    QName(b"aixm:designator") => designator = read_text(&mut reader, node.name)?,
                    QName(b"aixm:type") => point_type = Some(read_text(&mut reader, node.name)?),
                    QName(b"aixm:Point") => {
                        while let Ok(pos) = find_node(&mut reader, &[QName(b"gml:pos")], Some(node.name)) {
                            let coords: Vec<f64> = read_text(&mut reader, pos.name)?
                                .split_whitespace()
                                .filter_map(|s| s.parse::<f64>().ok())
                                .collect();
                            if coords.len() == 2 {
                                latitude = coords[0];
                                longitude = coords[1];
                            }
                        }
                    }
                    _ => {}
                }
            }

            if !designator.is_empty() {
                let code = designator.to_uppercase();
                out.push(NavpointRecord {
                    code: code.clone(),
                    identifier: code.clone(),
                    kind: "fix".to_string(),
                    name,
                    latitude,
                    longitude,
                    description: None,
                    frequency: None,
                    point_type,
                    region: None,
                    source: "eurocontrol_aixm".to_string(),
                });
                if !identifier.is_empty() {
                    by_id.insert(
                        identifier,
                        AirwayPointRecord {
                            code,
                            raw_code: designator.to_uppercase(),
                            kind: "fix".to_string(),
                            latitude,
                            longitude,
                        },
                    );
                }
            }
        }
    }
    Ok((out, by_id))
}

fn parse_aixm_navaids(zip_bytes: &[u8]) -> Result<NavpointsWithRefIndex, DynError> {
    let mut out = Vec::new();
    let mut by_id: HashMap<String, AirwayPointRecord> = HashMap::new();
    for xml in read_baseline_xml_documents(zip_bytes)? {
        let mut reader = Reader::from_reader(BufReader::new(Cursor::new(xml.into_bytes())));
        while find_node(&mut reader, &[QName(b"aixm:Navaid")], None).is_ok() {
            let mut identifier = String::new();
            let mut designator = None;
            let mut description = None;
            let mut point_type = None;
            let mut latitude = 0.0_f64;
            let mut longitude = 0.0_f64;
            while let Ok(node) = find_node(
                &mut reader,
                &[
                    QName(b"gml:identifier"),
                    QName(b"aixm:designator"),
                    QName(b"aixm:type"),
                    QName(b"aixm:name"),
                    QName(b"aixm:ElevatedPoint"),
                ],
                Some(QName(b"aixm:Navaid")),
            ) {
                match node.name {
                    QName(b"gml:identifier") => identifier = read_text(&mut reader, node.name)?,
                    QName(b"aixm:designator") => designator = Some(read_text(&mut reader, node.name)?),
                    QName(b"aixm:type") => point_type = Some(read_text(&mut reader, node.name)?),
                    QName(b"aixm:name") => description = Some(read_text(&mut reader, node.name)?),
                    QName(b"aixm:ElevatedPoint") => {
                        while let Ok(pos) = find_node(&mut reader, &[QName(b"gml:pos")], Some(node.name)) {
                            let coords: Vec<f64> = read_text(&mut reader, pos.name)?
                                .split_whitespace()
                                .filter_map(|s| s.parse::<f64>().ok())
                                .collect();
                            if coords.len() == 2 {
                                latitude = coords[0];
                                longitude = coords[1];
                            }
                        }
                    }
                    _ => {}
                }
            }

            if let Some(code) = designator {
                let upper = code.to_uppercase();
                out.push(NavpointRecord {
                    code: upper.clone(),
                    identifier: upper.clone(),
                    kind: "navaid".to_string(),
                    name: Some(code),
                    latitude,
                    longitude,
                    description,
                    frequency: None,
                    point_type,
                    region: None,
                    source: "eurocontrol_aixm".to_string(),
                });
                if !identifier.is_empty() {
                    by_id.insert(
                        identifier,
                        AirwayPointRecord {
                            code: upper.clone(),
                            raw_code: upper,
                            kind: "navaid".to_string(),
                            latitude,
                            longitude,
                        },
                    );
                }
            }
        }
    }
    Ok((out, by_id))
}

fn parse_aixm_airways(
    route_zip_bytes: &[u8],
    route_segment_zip_bytes: &[u8],
    points_by_id: &HashMap<String, AirwayPointRecord>,
) -> Result<Vec<AirwayRecord>, Box<dyn std::error::Error>> {
    let mut route_name_by_id: HashMap<String, String> = HashMap::new();
    for xml in read_baseline_xml_documents(route_zip_bytes)? {
        let mut reader = Reader::from_reader(BufReader::new(Cursor::new(xml.into_bytes())));
        while find_node(&mut reader, &[QName(b"aixm:Route")], None).is_ok() {
            let mut identifier = String::new();
            let mut prefix = String::new();
            let mut second = String::new();
            let mut number = String::new();
            let mut multiple = String::new();
            while let Ok(node) = find_node(
                &mut reader,
                &[
                    QName(b"gml:identifier"),
                    QName(b"aixm:designatorPrefix"),
                    QName(b"aixm:designatorSecondLetter"),
                    QName(b"aixm:designatorNumber"),
                    QName(b"aixm:multipleIdentifier"),
                ],
                Some(QName(b"aixm:Route")),
            ) {
                match node.name {
                    QName(b"gml:identifier") => identifier = read_text(&mut reader, node.name)?,
                    QName(b"aixm:designatorPrefix") => prefix = read_text(&mut reader, node.name)?,
                    QName(b"aixm:designatorSecondLetter") => second = read_text(&mut reader, node.name)?,
                    QName(b"aixm:designatorNumber") => number = read_text(&mut reader, node.name)?,
                    QName(b"aixm:multipleIdentifier") => multiple = read_text(&mut reader, node.name)?,
                    _ => {}
                }
            }
            if !identifier.is_empty() {
                route_name_by_id.insert(identifier, format!("{prefix}{second}{number}{multiple}").to_uppercase());
            }
        }
    }

    let mut grouped: HashMap<String, Vec<AirwayPointRecord>> = HashMap::new();
    for xml in read_baseline_xml_documents(route_segment_zip_bytes)? {
        let mut reader = Reader::from_reader(BufReader::new(Cursor::new(xml.into_bytes())));
        while find_node(&mut reader, &[QName(b"aixm:RouteSegment")], None).is_ok() {
            let mut route_id: Option<String> = None;
            let mut start_id: Option<String> = None;
            let mut end_id: Option<String> = None;

            while let Ok(node) = find_node(
                &mut reader,
                &[
                    QName(b"aixm:routeFormed"),
                    QName(b"aixm:start"),
                    QName(b"aixm:end"),
                    QName(b"aixm:pointChoice_fixDesignatedPoint"),
                    QName(b"aixm:pointChoice_navaidSystem"),
                    QName(b"aixm:extension"),
                    QName(b"aixm:annotation"),
                    QName(b"aixm:availability"),
                ],
                Some(QName(b"aixm:RouteSegment")),
            ) {
                let href_id = |key: &str| {
                    node.attributes
                        .get(key)
                        .map(|s| s.trim_start_matches("urn:uuid:").to_string())
                };
                match node.name {
                    QName(b"aixm:routeFormed") => route_id = href_id("xlink:href"),
                    QName(b"aixm:start") => {
                        while let Ok(point_node) = find_node(
                            &mut reader,
                            &[
                                QName(b"aixm:pointChoice_fixDesignatedPoint"),
                                QName(b"aixm:pointChoice_navaidSystem"),
                            ],
                            Some(QName(b"aixm:start")),
                        ) {
                            start_id = point_node
                                .attributes
                                .get("xlink:href")
                                .map(|s| s.trim_start_matches("urn:uuid:").to_string());
                        }
                    }
                    QName(b"aixm:end") => {
                        while let Ok(point_node) = find_node(
                            &mut reader,
                            &[
                                QName(b"aixm:pointChoice_fixDesignatedPoint"),
                                QName(b"aixm:pointChoice_navaidSystem"),
                            ],
                            Some(QName(b"aixm:end")),
                        ) {
                            end_id = point_node
                                .attributes
                                .get("xlink:href")
                                .map(|s| s.trim_start_matches("urn:uuid:").to_string());
                        }
                    }
                    _ => {}
                }
            }

            let Some(route_name) = route_id.and_then(|id| route_name_by_id.get(&id).cloned()) else {
                continue;
            };
            let Some(start) = start_id.and_then(|id| points_by_id.get(&id).cloned()) else {
                continue;
            };
            let Some(end) = end_id.and_then(|id| points_by_id.get(&id).cloned()) else {
                continue;
            };

            let entry = grouped.entry(route_name).or_default();
            if entry.last().map(|x| x.code.as_str()) != Some(start.code.as_str()) {
                entry.push(start);
            }
            if entry.last().map(|x| x.code.as_str()) != Some(end.code.as_str()) {
                entry.push(end);
            }
        }
    }

    Ok(grouped
        .into_iter()
        .map(|(name, points)| AirwayRecord {
            name,
            source: "eurocontrol_aixm".to_string(),
            points,
        })
        .collect())
}

fn parse_ddr_navpoints(text: &str) -> Vec<NavpointRecord> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(';').collect();
        if fields.len() < 5 {
            continue;
        }
        let lat = match fields[2].trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let lon = match fields[3].trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let code = fields[0].trim().to_uppercase();
        let point_type = fields[1].trim().to_uppercase();
        let kind = if point_type.contains("FIX") || point_type == "WPT" || point_type == "WP" {
            "fix"
        } else {
            "navaid"
        }
        .to_string();
        let description = fields
            .get(4)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "_");

        out.push(NavpointRecord {
            code: code.clone(),
            identifier: code,
            kind,
            name: description.clone(),
            latitude: lat,
            longitude: lon,
            description,
            frequency: None,
            point_type: Some(point_type),
            region: None,
            source: "eurocontrol_ddr".to_string(),
        });
    }
    out
}

fn parse_ddr_airports(text: &str) -> Vec<AirportRecord> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let code = parts[0].trim().to_uppercase();
        if code.len() != 4 {
            continue;
        }
        let lat_raw = match parts[1].parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let lon_raw = match parts[2].parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        out.push(AirportRecord {
            code: code.clone(),
            iata: None,
            icao: Some(code),
            name: None,
            latitude: lat_raw / 100.0,
            longitude: lon_raw / 100.0,
            region: None,
            source: "eurocontrol_ddr".to_string(),
        });
    }
    out
}

fn parse_ddr_airways(text: &str, point_lookup: &HashMap<String, (f64, f64, String)>) -> Vec<AirwayRecord> {
    let mut grouped: HashMap<String, Vec<(i32, AirwayPointRecord)>> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(';').collect();
        if fields.len() < 8 {
            continue;
        }
        let route = fields[1].trim().to_uppercase();
        let navaid = fields[5].trim().to_uppercase();
        let seq = fields[7].trim().parse::<i32>().unwrap_or(0);
        let (lat, lon, kind) = point_lookup
            .get(&navaid)
            .cloned()
            .unwrap_or((0.0, 0.0, "point".to_string()));

        grouped.entry(route).or_default().push((
            seq,
            AirwayPointRecord {
                code: navaid.clone(),
                raw_code: navaid,
                kind,
                latitude: lat,
                longitude: lon,
            },
        ));
    }

    let mut out = Vec::new();
    for (name, mut points) in grouped {
        points.sort_by_key(|(seq, _)| *seq);
        let deduped = points
            .into_iter()
            .map(|(_, p)| p)
            .fold(Vec::<AirwayPointRecord>::new(), |mut acc, p| {
                if acc.last().map(|x| x.code.as_str()) != Some(p.code.as_str()) {
                    acc.push(p);
                }
                acc
            });
        out.push(AirwayRecord {
            name,
            source: "eurocontrol_ddr".to_string(),
            points: deduped,
        });
    }
    out
}

#[wasm_bindgen]
pub struct EurocontrolResolver {
    airports: Vec<AirportRecord>,
    fixes: Vec<NavpointRecord>,
    navaids: Vec<NavpointRecord>,
    airways: Vec<AirwayRecord>,
    airport_index: HashMap<String, Vec<usize>>,
    fix_index: HashMap<String, Vec<usize>>,
    navaid_index: HashMap<String, Vec<usize>>,
    airway_index: HashMap<String, Vec<usize>>,
}

#[wasm_bindgen]
impl EurocontrolResolver {
    #[wasm_bindgen(constructor)]
    pub fn new(aixm_folder: JsValue) -> Result<EurocontrolResolver, JsValue> {
        let files: HashMap<String, Vec<u8>> =
            serde_wasm_bindgen::from_value(aixm_folder).map_err(|e| JsValue::from_str(&e.to_string()))?;
        for name in AIXM_EXPECTED_FILES {
            if !files.contains_key(name) {
                return Err(JsValue::from_str(&format!(
                    "missing AIXM file '{name}' in dataset folder payload"
                )));
            }
        }

        let airports = parse_aixm_airports(
            files
                .get("AirportHeliport.BASELINE.zip")
                .ok_or_else(|| JsValue::from_str("missing AirportHeliport.BASELINE.zip"))?,
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let (fixes, fixes_by_id) = parse_aixm_designated_points(
            files
                .get("DesignatedPoint.BASELINE.zip")
                .ok_or_else(|| JsValue::from_str("missing DesignatedPoint.BASELINE.zip"))?,
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let (navaids, navaids_by_id) = parse_aixm_navaids(
            files
                .get("Navaid.BASELINE.zip")
                .ok_or_else(|| JsValue::from_str("missing Navaid.BASELINE.zip"))?,
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let mut point_refs = fixes_by_id;
        point_refs.extend(navaids_by_id);
        let airways = parse_aixm_airways(
            files
                .get("Route.BASELINE.zip")
                .ok_or_else(|| JsValue::from_str("missing Route.BASELINE.zip"))?,
            files
                .get("RouteSegment.BASELINE.zip")
                .ok_or_else(|| JsValue::from_str("missing RouteSegment.BASELINE.zip"))?,
            &point_refs,
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Self::build(airports, fixes, navaids, airways)
    }

    #[wasm_bindgen(js_name = fromDdrFolder)]
    pub fn from_ddr_folder(ddr_folder: JsValue) -> Result<EurocontrolResolver, JsValue> {
        let files: HashMap<String, String> =
            serde_wasm_bindgen::from_value(ddr_folder).map_err(|e| JsValue::from_str(&e.to_string()))?;
        for name in DDR_EXPECTED_FILES {
            if !files.contains_key(name) {
                return Err(JsValue::from_str(&format!(
                    "missing DDR file '{name}' in dataset folder payload"
                )));
            }
        }

        let mut fixes = Vec::new();
        let mut navaids = Vec::new();

        let ddr_points = parse_ddr_navpoints(
            files
                .get("navpoints.nnpt")
                .ok_or_else(|| JsValue::from_str("missing navpoints.nnpt"))?,
        );
        for p in &ddr_points {
            if p.kind == "fix" {
                fixes.push(p.clone());
            } else {
                navaids.push(p.clone());
            }
        }

        let airports = parse_ddr_airports(
            files
                .get("airports.arp")
                .ok_or_else(|| JsValue::from_str("missing airports.arp"))?,
        );
        let mut point_lookup: HashMap<String, (f64, f64, String)> = HashMap::new();
        for p in &ddr_points {
            point_lookup.insert(p.code.clone(), (p.latitude, p.longitude, p.kind.clone()));
        }
        let airways = parse_ddr_airways(
            files
                .get("routes.routes")
                .ok_or_else(|| JsValue::from_str("missing routes.routes"))?,
            &point_lookup,
        );

        // Other DDR files are validated at input boundary so callers pass a complete
        // dataset folder; current resolver logic only consumes navpoints/routes/airports.

        Self::build(airports, fixes, navaids, airways)
    }

    fn build(
        airports: Vec<AirportRecord>,
        fixes: Vec<NavpointRecord>,
        navaids: Vec<NavpointRecord>,
        airways: Vec<AirwayRecord>,
    ) -> Result<EurocontrolResolver, JsValue> {
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

        let mut fix_index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, n) in fixes.iter().enumerate() {
            fix_index.entry(n.code.clone()).or_default().push(i);
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

        Ok(EurocontrolResolver {
            airports,
            fixes,
            navaids,
            airways,
            airport_index,
            fix_index,
            navaid_index,
            airway_index,
        })
    }

    pub fn airports(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.airports).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn fixes(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.fixes).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn navaids(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.navaids).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn airways(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.airways).map_err(|e| JsValue::from_str(&e.to_string()))
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
            .fix_index
            .get(&key)
            .and_then(|idx| idx.first().copied())
            .and_then(|i| self.fixes.get(i))
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
}
