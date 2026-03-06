use std::collections::{HashMap, HashSet};
use std::io::{BufReader, Cursor, Read};

use quick_xml::{events::Event, name::QName, Reader};
use thrust::data::eurocontrol::ddr::airspaces::{parse_are_bytes, parse_sls_bytes, DdrSectorLayer};
use wasm_bindgen::prelude::*;
use zip::read::ZipArchive;

use crate::models::{
    normalize_airway_name, AirportRecord, AirspaceCompositeRecord, AirspaceLayerRecord, AirspaceRecord,
    AirwayPointRecord, AirwayRecord, NavpointRecord,
};

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

// DDR routes are grouped by airway name, which can merge distinct route variants
// into a single sequence. We split a chain only when a consecutive leg is an
// extreme geodesic jump, using a conservative fixed cutoff. Empirical analysis
// on AIRAC_484E showed >=1000 NM gives near-zero same-name AIXM segment matches
// while still catching obvious merges (for example A10: *PR13 -> SIT).
const DDR_AIRWAY_SPLIT_GAP_NM: f64 = 1_000.0;

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
            route_class: None,
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
    fn decode_ddr_coords(lat_raw: f64, lon_raw: f64) -> Option<(f64, f64)> {
        if lat_raw.abs() <= 90.0 && lon_raw.abs() <= 180.0 {
            return Some((lat_raw, lon_raw));
        }

        let lat_minutes = lat_raw / 60.0;
        let lon_minutes = lon_raw / 60.0;
        if lat_minutes.abs() <= 90.0 && lon_minutes.abs() <= 180.0 {
            return Some((lat_minutes, lon_minutes));
        }

        let lat_scaled = lat_raw / 600_000.0;
        let lon_scaled = lon_raw / 600_000.0;
        if lat_scaled.abs() <= 90.0 && lon_scaled.abs() <= 180.0 {
            return Some((lat_scaled, lon_scaled));
        }

        None
    }

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
        let Some((latitude, longitude)) = decode_ddr_coords(lat_raw, lon_raw) else {
            continue;
        };

        out.push(AirportRecord {
            code: code.clone(),
            iata: None,
            icao: Some(code),
            name: None,
            latitude,
            longitude,
            region: None,
            source: "eurocontrol_ddr".to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{parse_ddr_airports, parse_ddr_airspaces, parse_ddr_airways};

    #[test]
    fn parse_lfbo_coordinates_from_ddr_arp() {
        let airports = parse_ddr_airports("LFBO 2618.100000 82.066667\n");
        let lfbo = airports.iter().find(|a| a.code == "LFBO").expect("LFBO not found");

        assert!((lfbo.latitude - 43.635).abs() < 1e-9);
        assert!((lfbo.longitude - 1.3677777833333334).abs() < 1e-9);
    }

    #[test]
    fn split_ddr_airway_on_very_large_gap() {
        let mut point_lookup: HashMap<String, (f64, f64, String)> = HashMap::new();
        point_lookup.insert("YJQ".to_string(), (10.0, 10.0, "fix".to_string()));
        point_lookup.insert("MITEK".to_string(), (10.0, 11.0, "fix".to_string()));
        point_lookup.insert("*PR13".to_string(), (10.0, 12.0, "point".to_string()));
        point_lookup.insert("SIT".to_string(), (55.0, 120.0, "fix".to_string()));
        point_lookup.insert("PAXIS".to_string(), (55.0, 121.0, "fix".to_string()));

        let text = [
            "L;A10;AR;999999999999;000000000000;YJQ;SP;1",
            "L;A10;AR;999999999999;000000000000;MITEK;SP;2",
            "L;A10;AR;999999999999;000000000000;*PR13;DBP;3",
            "L;A10;AR;999999999999;000000000000;SIT;SP;4",
            "L;A10;AR;999999999999;000000000000;PAXIS;SP;5",
        ]
        .join("\n");

        let airways = parse_ddr_airways(&text, &point_lookup);
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
        let mut point_lookup: HashMap<String, (f64, f64, String)> = HashMap::new();
        point_lookup.insert("A".to_string(), (43.6, 1.4, "fix".to_string()));
        point_lookup.insert("B".to_string(), (44.0, 2.0, "fix".to_string()));
        point_lookup.insert("C".to_string(), (44.5, 3.0, "fix".to_string()));

        let text = [
            "L;UM605;AR;999999999999;000000000000;A;SP;1",
            "L;UM605;AR;999999999999;000000000000;B;SP;2",
            "L;UM605;AR;999999999999;000000000000;C;SP;3",
        ]
        .join("\n");

        let airways = parse_ddr_airways(&text, &point_lookup);
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

fn parse_ddr_airways(text: &str, point_lookup: &HashMap<String, (f64, f64, String)>) -> Vec<AirwayRecord> {
    let mut grouped: HashMap<String, Vec<(i32, AirwayPointRecord, bool)>> = HashMap::new();
    let mut route_class_by_name: HashMap<String, String> = HashMap::new();
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
        let route_class = fields[2].trim().to_uppercase();
        let navaid = fields[5].trim().to_uppercase();
        let seq = fields[7].trim().parse::<i32>().unwrap_or(0);
        let (lat, lon, kind, has_coords) = match point_lookup.get(&navaid) {
            Some((lat, lon, kind)) => (*lat, *lon, kind.clone(), true),
            None => (0.0, 0.0, "point".to_string(), false),
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
    out
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
    );

    let airports = parse_ddr_airports(
        files
            .get("airports.arp")
            .ok_or_else(|| JsValue::from_str("missing airports.arp"))?,
    );
    let mut point_lookup: HashMap<String, (f64, f64, String)> = HashMap::new();
    for p in &navaids {
        point_lookup.insert(p.code.clone(), (p.latitude, p.longitude, p.kind.clone()));
    }
    let airways = parse_ddr_airways(
        files
            .get("routes.routes")
            .ok_or_else(|| JsValue::from_str("missing routes.routes"))?,
        &point_lookup,
    );
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
        let (designated_points, designated_points_by_id) = parse_aixm_designated_points(
            files
                .get("DesignatedPoint.BASELINE.zip")
                .ok_or_else(|| JsValue::from_str("missing DesignatedPoint.BASELINE.zip"))?,
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let (mut navaids, navaids_by_id) = parse_aixm_navaids(
            files
                .get("Navaid.BASELINE.zip")
                .ok_or_else(|| JsValue::from_str("missing Navaid.BASELINE.zip"))?,
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
        navaids.extend(designated_points);
        let mut point_refs = designated_points_by_id;
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

        Self::build(airports, navaids, airways, Vec::new())
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
}
