use crate::error::ThrustError;
use quick_xml::name::QName;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use zip::read::ZipArchive;

use crate::data::eurocontrol::aixm::Node;

use super::{find_node, read_text};

/// A single segment of an ATS route connecting two sequential navigation points.
///
/// Route segments form the building blocks of complete airways. Each segment
/// connects a start point and end point, with optional availability restrictions.
/// Full routes are constructed by chaining multiple segments together in sequence.
///
/// # Fields
/// - `identifier`: Unique database key
/// - `route_formed`: The parent route designator this segment belongs to
/// - `start`: Departure point (navaid, waypoint, or airport)
/// - `end`: Arrival point (navaid, waypoint, or airport)
///
/// # Example
/// ```ignore
/// let segment = RouteSegment {
///     identifier: "SEG001".to_string(),
///     route_formed: Some("N100".to_string()),
///     start: PointReference::Navaid("SEA".to_string()),
///     end: PointReference::DesignatedPoint("APTIN".to_string()),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RouteSegment {
    #[serde(skip)]
    pub identifier: String,
    /// The identifier of the route this segment belongs to
    pub route_formed: Option<String>,
    /// Starting point of the segment
    pub start: PointReference,
    /// Ending point of the segment
    pub end: PointReference,
    // the following fields are related to availabilities, which are not properly modelled yet
    // pub lower_limit: Option<String>,
    // pub upper_limit: Option<String>,
    // pub direction: Option<String>,
}

/// A reference to a navigation point (designated point, navaid, or airport).
///
/// Used in route segments, procedure legs, and airspace definitions to reference
/// specific navigation points. The enum variants represent different point types
/// that can appear in aviation procedures and routes.
///
/// # Variants
/// - `DesignatedPoint(String)`: Published waypoint/fix identifier (e.g., "APTIN")
/// - `Navaid(String)`: Navigation aid identifier (e.g., "SEA" for VOR)
/// - `AirportHeliport(String)`: Airport or heliport identifier (e.g., "KSEA")
/// - `None`: Point not resolved or undefined
///
/// # Example
/// ```ignore
/// let point = PointReference::DesignatedPoint("APTIN".to_string());
/// match point {
///     PointReference::DesignatedPoint(name) => println!("Waypoint: {}", name),
///     _ => println!("Other point type"),
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum PointReference {
    DesignatedPoint(String),
    Navaid(String),
    AirportHeliport(String),
    #[default]
    None,
}

impl PointReference {
    pub fn name(&self) -> String {
        match self {
            PointReference::DesignatedPoint(id) => id.to_string(),
            PointReference::Navaid(id) => id.to_string(),
            PointReference::AirportHeliport(id) => id.to_string(),
            PointReference::None => "".to_string(),
        }
    }

    pub fn is_airport_heliport(&self) -> bool {
        matches!(self, PointReference::AirportHeliport(_))
    }
}

/**
 * Parse route segment data from a ZIP file containing AIXM data.
 */
pub fn parse_route_segment_zip_file<P: AsRef<Path>>(path: P) -> Result<HashMap<String, RouteSegment>, ThrustError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut route_segments = HashMap::new();

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.name().ends_with(".BASELINE") {
            let mut reader = Reader::from_reader(BufReader::new(file));

            while let Ok(_node) = find_node(&mut reader, vec![QName(b"aixm:RouteSegment")], None) {
                let route_segment = parse_route_segment(&mut reader)?;
                route_segments.insert(route_segment.identifier.clone(), route_segment);
            }
        }
    }

    Ok(route_segments)
}

fn parse_route_segment<R: std::io::BufRead>(reader: &mut Reader<R>) -> Result<RouteSegment, ThrustError> {
    let mut segment = RouteSegment::default();

    while let Ok(node) = find_node(
        reader,
        vec![
            QName(b"gml:identifier"),
            QName(b"aixm:routeFormed"),
            QName(b"aixm:start"),
            QName(b"aixm:end"),
            //QName(b"aixm:lowerLimit"),
            //QName(b"aixm:upperLimit"),
            //QName(b"aixm:direction"),
            QName(b"aixm:extension"),
            QName(b"aixm:annotation"),
            QName(b"aixm:availability"),
        ],
        Some(QName(b"aixm:RouteSegment")),
    ) {
        let Node { name, attributes } = node;
        match name {
            QName(b"gml:identifier") => {
                segment.identifier = read_text(reader, name)?;
            }
            QName(b"aixm:extension") | QName(b"aixm:availability") | QName(b"aixm_annotation") => {
                // Skip the whole block
                let _ = find_node(reader, vec![], Some(name));
            }
            QName(b"aixm:routeFormed") => {
                if let Some(id) = attributes
                    .get("xlink:href")
                    .map(|s| s.strip_prefix("urn:uuid:").unwrap_or(s))
                {
                    segment.route_formed = Some(id.to_string());
                }
            }
            /*QName(b"aixm:lowerLimit") => {
                segment.lower_limit = Some(read_text(reader, node)?);
            }
            QName(b"aixm:upperLimit") => {
                segment.upper_limit = Some(read_text(reader, node)?);
            }
            QName(b"aixm:direction") => {
                segment.direction = Some(read_text(reader, node)?);
            }*/
            QName(b"aixm:start") => {
                while let Ok(node) = find_node(
                    reader,
                    vec![
                        QName(b"aixm:pointChoice_fixDesignatedPoint"),
                        QName(b"aixm:pointChoice_navaidSystem"),
                    ],
                    Some(name),
                ) {
                    let Node { name, attributes } = node;
                    match name {
                        QName(b"aixm:pointChoice_fixDesignatedPoint") => {
                            if let Some(id) = attributes
                                .get("xlink:href")
                                .map(|s| s.strip_prefix("urn:uuid:").unwrap_or(s))
                            {
                                segment.start = PointReference::DesignatedPoint(id.to_string());
                            }
                        }
                        QName(b"aixm:pointChoice_navaidSystem") => {
                            if let Some(id) = attributes
                                .get("xlink:href")
                                .map(|s| s.strip_prefix("urn:uuid:").unwrap_or(s))
                            {
                                segment.start = PointReference::Navaid(id.to_string());
                            }
                        }
                        _ => (),
                    }
                }
            }
            QName(b"aixm:end") => {
                while let Ok(node) = find_node(
                    reader,
                    vec![
                        QName(b"aixm:pointChoice_fixDesignatedPoint"),
                        QName(b"aixm:pointChoice_navaidSystem"),
                    ],
                    Some(name),
                ) {
                    let Node { name, attributes } = node;
                    match name {
                        QName(b"aixm:pointChoice_fixDesignatedPoint") => {
                            if let Some(id) = attributes
                                .get("xlink:href")
                                .map(|s| s.strip_prefix("urn:uuid:").unwrap_or(s))
                            {
                                segment.end = PointReference::DesignatedPoint(id.to_string());
                            }
                        }
                        QName(b"aixm:pointChoice_navaidSystem") => {
                            if let Some(id) = attributes
                                .get("xlink:href")
                                .map(|s| s.strip_prefix("urn:uuid:").unwrap_or(s))
                            {
                                segment.end = PointReference::Navaid(id.to_string());
                            }
                        }
                        _ => (),
                    }
                }
            }
            _ => (),
        }
    }
    Ok(segment)
}
