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

/**
 * A designated point as defined in AIXM.
 *
 * These are waypoints that are not navaids.
 */
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DesignatedPoint {
    #[serde(skip)]
    pub identifier: String,
    /// Latitude in decimal degrees
    pub latitude: f64,
    /// Longitude in decimal degrees
    pub longitude: f64,
    #[serde(rename = "name")]
    /// Name of the designated point
    pub designator: String,
    #[serde(skip)]
    pub name: Option<String>,
    #[serde(skip)]
    /// Type of designated point (TODO: enum?)
    pub r#type: String,
}

pub fn parse_designated_point_zip_file<P: AsRef<Path>>(
    path: P,
) -> Result<HashMap<String, DesignatedPoint>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut points = HashMap::new();

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.name().ends_with(".BASELINE") {
            let mut reader = Reader::from_reader(BufReader::new(file));

            while let Ok(_nome) = find_node(&mut reader, vec![QName(b"aixm:DesignatedPoint")], None) {
                let point = parse_designated_point(&mut reader)?;
                points.insert(point.identifier.clone(), point);
            }
        }
    }

    Ok(points)
}

fn parse_designated_point<R: std::io::BufRead>(
    reader: &mut Reader<R>,
) -> Result<DesignatedPoint, Box<dyn std::error::Error>> {
    let mut point = DesignatedPoint::default();

    while let Ok(node) = find_node(
        reader,
        vec![
            QName(b"gml:identifier"),
            QName(b"aixm:name"),
            QName(b"aixm:designator"),
            QName(b"aixm:type"),
            QName(b"aixm:Point"),
        ],
        Some(QName(b"aixm:DesignatedPoint")),
    ) {
        let Node { name, .. } = node;
        match name {
            QName(b"gml:identifier") => {
                point.identifier = read_text(reader, name)?;
            }
            QName(b"aixm:name") => {
                point.name = Some(read_text(reader, name)?);
            }
            QName(b"aixm:designator") => {
                point.designator = read_text(reader, name)?;
            }
            QName(b"aixm:type") => {
                point.r#type = read_text(reader, name)?;
            }
            QName(b"aixm:Point") => {
                while let Ok(node) = find_node(reader, vec![QName(b"gml:pos")], Some(name)) {
                    let Node { name, .. } = node;
                    let coords: Vec<f64> = read_text(reader, name)?
                        .split_whitespace()
                        .map(|s| s.parse().unwrap())
                        .collect();
                    point.latitude = coords[0];
                    point.longitude = coords[1];
                }
            }
            _ => (),
        }
    }

    Ok(point)
}
