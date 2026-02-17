use quick_xml::name::QName;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use zip::read::ZipArchive;

use crate::data::eurocontrol::aixm::route_segment::PointReference;
use crate::data::eurocontrol::aixm::Node;

use super::{find_node, read_text};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StandardInstrumentDeparture {
    #[serde(skip)]
    pub identifier: String,
    pub designator: String,
    pub airport_heliport: Option<String>,
    pub instruction: Option<String>,
    pub connecting_points: Vec<PointReference>,
}

pub fn parse_standard_instrument_departure_zip_file<P: AsRef<Path>>(
    path: P,
) -> Result<HashMap<String, StandardInstrumentDeparture>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut departures = HashMap::new();

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.name().ends_with(".BASELINE") {
            let mut reader = Reader::from_reader(BufReader::new(file));

            while let Ok(_node) = find_node(&mut reader, vec![QName(b"aixm:StandardInstrumentDeparture")], None) {
                let departure = parse_standard_instrument_departure(&mut reader)?;
                departures.insert(departure.identifier.clone(), departure);
            }
        }
    }

    Ok(departures)
}

fn parse_standard_instrument_departure<R: std::io::BufRead>(
    reader: &mut Reader<R>,
) -> Result<StandardInstrumentDeparture, Box<dyn std::error::Error>> {
    let mut departure = StandardInstrumentDeparture::default();

    while let Ok(node) = find_node(
        reader,
        vec![
            QName(b"gml:identifier"),
            QName(b"aixm:airportHeliport"),
            QName(b"aixm:designator"),
            QName(b"aixm:instruction"),
            QName(b"aixm:extension"),
        ],
        Some(QName(b"aixm:StandardInstrumentDeparture")),
    ) {
        let Node { name, attributes } = node;
        match name {
            QName(b"gml:identifier") => {
                departure.identifier = read_text(reader, name)?;
            }
            QName(b"aixm:airportHeliport") => {
                departure.airport_heliport = extract_uuid_href(&attributes);
            }
            QName(b"aixm:designator") => {
                departure.designator = read_text(reader, name)?;
            }
            QName(b"aixm:instruction") => {
                departure.instruction = Some(read_text(reader, name)?);
            }
            QName(b"aixm:extension") => {
                while let Ok(node) = find_node(
                    reader,
                    vec![QName(b"adrext:connectingPoint")],
                    Some(QName(b"aixm:extension")),
                ) {
                    if let Some(point) = parse_connecting_point(reader, node.name)? {
                        departure.connecting_points.push(point);
                    }
                }
            }
            _ => (),
        }
    }

    Ok(departure)
}

fn parse_connecting_point<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    end: QName,
) -> Result<Option<PointReference>, Box<dyn std::error::Error>> {
    while let Ok(node) = find_node(reader, vec![QName(b"aixm:TerminalSegmentPoint")], Some(end)) {
        while let Ok(node) = find_node(
            reader,
            vec![
                QName(b"aixm:pointChoice_fixDesignatedPoint"),
                QName(b"aixm:pointChoice_navaidSystem"),
            ],
            Some(node.name),
        ) {
            let Node { name, attributes } = node;
            if let Some(id) = extract_uuid_href(&attributes) {
                return Ok(Some(match name {
                    QName(b"aixm:pointChoice_fixDesignatedPoint") => PointReference::DesignatedPoint(id),
                    QName(b"aixm:pointChoice_navaidSystem") => PointReference::Navaid(id),
                    _ => PointReference::None,
                }));
            }
        }
    }

    Ok(None)
}

fn extract_uuid_href(attributes: &HashMap<String, String>) -> Option<String> {
    attributes
        .get("xlink:href")
        .map(|s| s.strip_prefix("urn:uuid:").unwrap_or(s).to_string())
}
