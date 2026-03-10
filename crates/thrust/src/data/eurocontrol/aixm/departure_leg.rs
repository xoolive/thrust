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

/// A single segment of a Standard Instrument Departure (SID) procedure.
///
/// Connects two navigation points in a terminal departure procedure.
/// Part of the SID that guides aircraft from the airport through the
/// departure phase into the en route network.
///
/// # Fields
/// - `identifier`: Unique identifier for this leg
/// - `departure`: Associated SID identifier (e.g., "KSEA01")
/// - `start`: Entry point (airport or initial waypoint)
/// - `end`: Exit point to the next leg or navaid
///
/// # Example
/// ```ignore
/// let leg = DepartureLeg {
///     identifier: "LEG001".to_string(),
///     departure: Some("KSEA01".to_string()),
///     start: PointReference::Airport("KSEA".to_string()),
///     end: PointReference::DesignatedPoint("KENRY".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DepartureLeg {
    pub identifier: String,
    pub departure: Option<String>,
    pub start: PointReference,
    pub end: PointReference,
}

pub fn parse_departure_leg_zip_file<P: AsRef<Path>>(path: P) -> Result<HashMap<String, DepartureLeg>, ThrustError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut legs = HashMap::new();

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.name().ends_with(".BASELINE") {
            let mut reader = Reader::from_reader(BufReader::new(file));

            while let Ok(_node) = find_node(&mut reader, vec![QName(b"aixm:DepartureLeg")], None) {
                let leg = parse_departure_leg(&mut reader)?;
                legs.insert(leg.identifier.clone(), leg);
            }
        }
    }

    Ok(legs)
}

fn parse_departure_leg<R: std::io::BufRead>(reader: &mut Reader<R>) -> Result<DepartureLeg, ThrustError> {
    let mut leg = DepartureLeg::default();

    while let Ok(node) = find_node(
        reader,
        vec![
            QName(b"gml:identifier"),
            QName(b"aixm:startPoint"),
            QName(b"aixm:endPoint"),
            QName(b"aixm:departure"),
        ],
        Some(QName(b"aixm:DepartureLeg")),
    ) {
        let Node { name, attributes } = node;
        match name {
            QName(b"gml:identifier") => {
                leg.identifier = read_text(reader, name)?;
            }
            QName(b"aixm:startPoint") => {
                leg.start = parse_terminal_segment_point(reader, name)?;
            }
            QName(b"aixm:endPoint") => {
                leg.end = parse_terminal_segment_point(reader, name)?;
            }
            QName(b"aixm:departure") => {
                leg.departure = attributes
                    .get("xlink:href")
                    .map(|s| s.strip_prefix("urn:uuid:").unwrap_or(s).to_string());
            }
            _ => (),
        }
    }
    Ok(leg)
}

fn parse_terminal_segment_point<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    end: QName,
) -> Result<PointReference, ThrustError> {
    while let Ok(node) = find_node(reader, vec![QName(b"aixm:TerminalSegmentPoint")], Some(end)) {
        while let Ok(node) = find_node(
            reader,
            vec![
                QName(b"aixm:pointChoice_fixDesignatedPoint"),
                QName(b"aixm:pointChoice_navaidSystem"),
                QName(b"aixm:pointChoice_airportReferencePoint"),
            ],
            Some(node.name),
        ) {
            let Node { name, attributes } = node;
            if let Some(id) = attributes
                .get("xlink:href")
                .map(|s| s.strip_prefix("urn:uuid:").unwrap_or(s).to_string())
            {
                return Ok(match name {
                    QName(b"aixm:pointChoice_fixDesignatedPoint") => PointReference::DesignatedPoint(id),
                    QName(b"aixm:pointChoice_navaidSystem") => PointReference::Navaid(id),
                    QName(b"aixm:pointChoice_airportReferencePoint") => PointReference::AirportHeliport(id),
                    _ => PointReference::None,
                });
            }
        }
    }
    Ok(PointReference::None)
}
