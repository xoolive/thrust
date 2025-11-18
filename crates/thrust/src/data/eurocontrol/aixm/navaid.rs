use quick_xml::name::QName;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::io::BufReader;
use std::path::Path;
use std::{collections::HashMap, fs::File};
use zip::read::ZipArchive;

use crate::data::eurocontrol::aixm::Node;

use super::{find_node, read_text};

/**
 * A navaid as defined in AIXM.
 *
 */
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Navaid {
    #[serde(skip)]
    pub identifier: String,
    /// Latitude in decimal degrees
    pub latitude: f64,
    /// Longitude in decimal degrees
    pub longitude: f64,
    /// Name of the navaid (e.g., VOR identifier)
    pub name: Option<String>,
    #[serde(skip)]
    /// Type of navaid (e.g., VOR, NDB, etc.) TODO: enum?
    pub r#type: String,
    #[serde(skip)]
    /// Textual description of the navaid
    pub description: Option<String>,
}

pub fn parse_navaid_zip_file<P: AsRef<Path>>(path: P) -> Result<HashMap<String, Navaid>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut navaids = HashMap::new();

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.name().ends_with(".BASELINE") {
            let mut reader = Reader::from_reader(BufReader::new(file));

            while let Ok(_node) = find_node(&mut reader, vec![QName(b"aixm:Navaid")], None) {
                let navaid = parse_navaid(&mut reader)?;
                navaids.insert(navaid.identifier.clone(), navaid);
            }
        }
    }

    Ok(navaids)
}

fn parse_navaid<R: std::io::BufRead>(reader: &mut Reader<R>) -> Result<Navaid, Box<dyn std::error::Error>> {
    let mut navaid = Navaid::default();

    while let Ok(node) = find_node(
        reader,
        vec![
            QName(b"gml:identifier"),
            QName(b"aixm:designator"),
            QName(b"aixm:type"),
            QName(b"aixm:name"),
            QName(b"aixm:ElevatedPoint"),
        ],
        Some(QName(b"aixm:Navaid")),
    ) {
        let Node { name, .. } = node;
        match name {
            QName(b"gml:identifier") => {
                navaid.identifier = read_text(reader, name)?;
            }
            QName(b"aixm:designator") => {
                navaid.name = Some(read_text(reader, name)?);
            }
            QName(b"aixm:type") => {
                navaid.r#type = read_text(reader, name)?;
            }
            QName(b"aixm:name") => {
                navaid.description = Some(read_text(reader, name)?);
            }
            QName(b"aixm:ElevatedPoint") => {
                while let Ok(node) = find_node(reader, vec![QName(b"gml:pos")], Some(name)) {
                    let Node { name, .. } = node;
                    let coords: Vec<f64> = read_text(reader, name)?
                        .split_whitespace()
                        .map(|s| s.parse().unwrap())
                        .collect();
                    navaid.latitude = coords[0];
                    navaid.longitude = coords[1];
                }
            }
            _ => (),
        }
    }

    Ok(navaid)
}
