use crate::error::ThrustError;
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

/// A Standard Instrument Departure (SID) procedure.
///
/// A SID is a published procedure that guides departing aircraft from the airport
/// into the en route structure. Each SID consists of one or more departure legs
/// connecting navigation points and defining the departure transition.
///
/// # Fields
/// - `identifier`: Unique database key
/// - `designator`: Published procedure name (e.g., "KSEA05", "RCKT2")
/// - `airport_heliport`: Departure airport/heliport identifier
/// - `instruction`: Operating procedure notes or restrictions
/// - `connecting_points`: Sequence of waypoints and navaids defining the departure
///
/// # Example
/// ```ignore
/// let sid = StandardInstrumentDeparture {
///     identifier: "SID001".to_string(),
///     designator: "KSEA05".to_string(),
///     airport_heliport: Some("KSEA".to_string()),
///     connecting_points: vec![
///         PointReference::Airport("KSEA".to_string()),
///         PointReference::DesignatedPoint("KENRY".to_string()),
///     ],
///     ..Default::default()
/// };
/// ```
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
) -> Result<HashMap<String, StandardInstrumentDeparture>, ThrustError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut departures = HashMap::new();

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.name().ends_with(".BASELINE") {
            let mut reader = Reader::from_reader(BufReader::new(file));

            while let Ok(_node) = find_node(&mut reader, vec![QName("aixm:StandardInstrumentDeparture")], None) {
                let departure = parse_standard_instrument_departure(&mut reader)?;
                departures.insert(departure.identifier.clone(), departure);
            }
        }
    }

    Ok(departures)
}

fn parse_standard_instrument_departure<R: std::io::BufRead>(
    reader: &mut Reader<R>,
) -> Result<StandardInstrumentDeparture, ThrustError> {
    let mut departure = StandardInstrumentDeparture::default();

    while let Ok(node) = find_node(
        reader,
        vec![
            QName("gml:identifier"),
            QName("aixm:airportHeliport"),
            QName("aixm:designator"),
            QName("aixm:instruction"),
            QName("aixm:extension"),
        ],
        Some(QName("aixm:StandardInstrumentDeparture")),
    ) {
        let Node { name, attributes } = node;
        match name {
            QName("gml:identifier") => {
                departure.identifier = read_text(reader, name)?;
            }
            QName("aixm:airportHeliport") => {
                departure.airport_heliport = extract_uuid_href(&attributes);
            }
            QName("aixm:designator") => {
                departure.designator = read_text(reader, name)?;
            }
            QName("aixm:instruction") => {
                departure.instruction = Some(read_text(reader, name)?);
            }
            QName("aixm:extension") => {
                while let Ok(node) = find_node(
                    reader,
                    vec![QName("adrext:connectingPoint")],
                    Some(QName("aixm:extension")),
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
) -> Result<Option<PointReference>, ThrustError> {
    while let Ok(node) = find_node(reader, vec![QName("aixm:TerminalSegmentPoint")], Some(end)) {
        while let Ok(node) = find_node(
            reader,
            vec![
                QName("aixm:pointChoice_fixDesignatedPoint"),
                QName("aixm:pointChoice_navaidSystem"),
            ],
            Some(node.name),
        ) {
            let Node { name, attributes } = node;
            if let Some(id) = extract_uuid_href(&attributes) {
                return Ok(Some(match name {
                    QName("aixm:pointChoice_fixDesignatedPoint") => PointReference::DesignatedPoint(id),
                    QName("aixm:pointChoice_navaidSystem") => PointReference::Navaid(id),
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
