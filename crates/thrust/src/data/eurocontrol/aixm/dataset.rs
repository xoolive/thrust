use std::collections::HashMap;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;

use quick_xml::name::QName;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use zip::read::ZipArchive;

use crate::error::ThrustError;

use super::{find_node, read_text};

const AIXM_REQUIRED_FILES: [&str; 5] = [
    "AirportHeliport.BASELINE.zip",
    "Navaid.BASELINE.zip",
    "DesignatedPoint.BASELINE.zip",
    "Route.BASELINE.zip",
    "RouteSegment.BASELINE.zip",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AirportRecord {
    pub code: String,
    pub iata: Option<String>,
    pub icao: Option<String>,
    pub name: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub region: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NavpointRecord {
    pub code: String,
    pub identifier: String,
    pub kind: String,
    pub name: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub description: Option<String>,
    pub frequency: Option<f64>,
    pub point_type: Option<String>,
    pub region: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AirwayPointRecord {
    pub code: String,
    pub raw_code: String,
    pub kind: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AirwayRecord {
    pub name: String,
    pub source: String,
    pub route_class: Option<String>,
    pub points: Vec<AirwayPointRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AixmDataset {
    pub airports: Vec<AirportRecord>,
    pub navaids: Vec<NavpointRecord>,
    pub airways: Vec<AirwayRecord>,
}

type PointRefIndex = HashMap<String, AirwayPointRecord>;
type NavpointsWithRefIndex = (Vec<NavpointRecord>, PointRefIndex);

pub fn parse_aixm_folder_path<P: AsRef<Path>>(path: P) -> Result<AixmDataset, ThrustError> {
    let base = path.as_ref();
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();

    for name in AIXM_REQUIRED_FILES {
        let bytes = std::fs::read(base.join(name))?;
        files.insert(name.to_string(), bytes);
    }

    parse_aixm_folder_bytes(&files)
}

pub fn parse_aixm_folder_bytes(files: &HashMap<String, Vec<u8>>) -> Result<AixmDataset, ThrustError> {
    for name in AIXM_REQUIRED_FILES {
        if !files.contains_key(name) {
            return Err(ThrustError::MissingField(format!(
                "missing AIXM file '{name}' in dataset payload"
            )));
        }
    }

    let airports = parse_aixm_airports(
        files
            .get("AirportHeliport.BASELINE.zip")
            .ok_or_else(|| ThrustError::MissingField("missing AirportHeliport.BASELINE.zip".to_string()))?,
    )?;

    let (designated_points, designated_points_by_id) = parse_aixm_designated_points(
        files
            .get("DesignatedPoint.BASELINE.zip")
            .ok_or_else(|| ThrustError::MissingField("missing DesignatedPoint.BASELINE.zip".to_string()))?,
    )?;

    let (mut navaids, navaids_by_id) = parse_aixm_navaids(
        files
            .get("Navaid.BASELINE.zip")
            .ok_or_else(|| ThrustError::MissingField("missing Navaid.BASELINE.zip".to_string()))?,
    )?;

    navaids.extend(designated_points);
    let mut point_refs = designated_points_by_id;
    point_refs.extend(navaids_by_id);

    let airways = parse_aixm_airways(
        files
            .get("Route.BASELINE.zip")
            .ok_or_else(|| ThrustError::MissingField("missing Route.BASELINE.zip".to_string()))?,
        files
            .get("RouteSegment.BASELINE.zip")
            .ok_or_else(|| ThrustError::MissingField("missing RouteSegment.BASELINE.zip".to_string()))?,
        &point_refs,
    )?;

    Ok(AixmDataset {
        airports,
        navaids,
        airways,
    })
}

fn read_baseline_xml_documents(zip_bytes: &[u8]) -> Result<Vec<String>, ThrustError> {
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

fn parse_aixm_airports(zip_bytes: &[u8]) -> Result<Vec<AirportRecord>, ThrustError> {
    let mut out = Vec::new();
    for xml in read_baseline_xml_documents(zip_bytes)? {
        let mut reader = Reader::from_reader(BufReader::new(Cursor::new(xml.into_bytes())));
        while find_node(&mut reader, vec![QName(b"aixm:AirportHeliport")], None).is_ok() {
            let mut icao = String::new();
            let mut iata = None;
            let mut name = String::new();
            let mut latitude = 0.0_f64;
            let mut longitude = 0.0_f64;
            while let Ok(node) = find_node(
                &mut reader,
                vec![
                    QName(b"aixm:locationIndicatorICAO"),
                    QName(b"aixm:designatorIATA"),
                    QName(b"aixm:name"),
                    QName(b"aixm:ElevatedPoint"),
                ],
                Some(QName(b"aixm:AirportHeliport")),
            ) {
                match node.name {
                    QName(b"aixm:locationIndicatorICAO") => icao = read_text(&mut reader, node.name)?,
                    QName(b"aixm:designatorIATA") => iata = Some(read_text(&mut reader, node.name)?),
                    QName(b"aixm:name") => name = read_text(&mut reader, node.name)?,
                    QName(b"aixm:ElevatedPoint") => {
                        while let Ok(pos) =
                            find_node(&mut reader, vec![QName(b"gml:pos")], Some(QName(b"aixm:ElevatedPoint")))
                        {
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
            }
        }
    }
    Ok(out)
}

fn parse_aixm_designated_points(zip_bytes: &[u8]) -> Result<NavpointsWithRefIndex, ThrustError> {
    let mut out = Vec::new();
    let mut by_id: HashMap<String, AirwayPointRecord> = HashMap::new();
    for xml in read_baseline_xml_documents(zip_bytes)? {
        let mut reader = Reader::from_reader(BufReader::new(Cursor::new(xml.into_bytes())));
        while find_node(&mut reader, vec![QName(b"aixm:DesignatedPoint")], None).is_ok() {
            let mut identifier = String::new();
            let mut designator = String::new();
            let mut name = None;
            let mut latitude = 0.0_f64;
            let mut longitude = 0.0_f64;
            let mut point_type = None;
            while let Ok(node) = find_node(
                &mut reader,
                vec![
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
                        while let Ok(pos) = find_node(&mut reader, vec![QName(b"gml:pos")], Some(node.name)) {
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

fn parse_aixm_navaids(zip_bytes: &[u8]) -> Result<NavpointsWithRefIndex, ThrustError> {
    let mut out = Vec::new();
    let mut by_id: HashMap<String, AirwayPointRecord> = HashMap::new();
    for xml in read_baseline_xml_documents(zip_bytes)? {
        let mut reader = Reader::from_reader(BufReader::new(Cursor::new(xml.into_bytes())));
        while find_node(&mut reader, vec![QName(b"aixm:Navaid")], None).is_ok() {
            let mut identifier = String::new();
            let mut designator = None;
            let mut description = None;
            let mut point_type = None;
            let mut latitude = 0.0_f64;
            let mut longitude = 0.0_f64;
            while let Ok(node) = find_node(
                &mut reader,
                vec![
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
                        while let Ok(pos) = find_node(&mut reader, vec![QName(b"gml:pos")], Some(node.name)) {
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
) -> Result<Vec<AirwayRecord>, ThrustError> {
    let mut route_name_by_id: HashMap<String, String> = HashMap::new();
    for xml in read_baseline_xml_documents(route_zip_bytes)? {
        let mut reader = Reader::from_reader(BufReader::new(Cursor::new(xml.into_bytes())));
        while find_node(&mut reader, vec![QName(b"aixm:Route")], None).is_ok() {
            let mut identifier = String::new();
            let mut prefix = String::new();
            let mut second = String::new();
            let mut number = String::new();
            let mut multiple = String::new();
            while let Ok(node) = find_node(
                &mut reader,
                vec![
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
        while find_node(&mut reader, vec![QName(b"aixm:RouteSegment")], None).is_ok() {
            let mut route_id: Option<String> = None;
            let mut start_id: Option<String> = None;
            let mut end_id: Option<String> = None;

            while let Ok(node) = find_node(
                &mut reader,
                vec![
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
                            vec![
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
                            vec![
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
