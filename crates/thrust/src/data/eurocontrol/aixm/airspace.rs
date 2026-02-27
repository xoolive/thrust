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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AirspaceVolume {
    pub upper_limit: Option<String>,
    pub upper_limit_reference: Option<String>,
    pub lower_limit: Option<String>,
    pub lower_limit_reference: Option<String>,
    pub polygon: Vec<(f64, f64)>,
    pub point_refs: Vec<String>,
    pub component_airspace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Airspace {
    #[serde(skip)]
    pub identifier: String,
    pub designator: Option<String>,
    pub type_: Option<String>,
    pub name: Option<String>,
    pub volumes: Vec<AirspaceVolume>,
}

pub fn parse_airspace_zip_file<P: AsRef<Path>>(
    path: P,
) -> Result<HashMap<String, Airspace>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut airspaces = HashMap::new();

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.name().ends_with(".BASELINE") {
            let mut reader = Reader::from_reader(BufReader::new(file));

            while let Ok(_node) = find_node(&mut reader, vec![QName(b"aixm:Airspace")], None) {
                let airspace = parse_airspace(&mut reader)?;
                airspaces.insert(airspace.identifier.clone(), airspace);
            }
        }
    }

    Ok(airspaces)
}

fn parse_airspace<R: std::io::BufRead>(reader: &mut Reader<R>) -> Result<Airspace, Box<dyn std::error::Error>> {
    let mut airspace = Airspace::default();

    while let Ok(node) = find_node(
        reader,
        vec![
            QName(b"gml:identifier"),
            QName(b"aixm:designator"),
            QName(b"aixm:type"),
            QName(b"aixm:name"),
            QName(b"aixm:AirspaceVolume"),
        ],
        Some(QName(b"aixm:Airspace")),
    ) {
        let Node { name, .. } = node;
        match name {
            QName(b"gml:identifier") => {
                airspace.identifier = read_text(reader, name)?;
            }
            QName(b"aixm:designator") => {
                airspace.designator = Some(read_text(reader, name)?);
            }
            QName(b"aixm:type") => {
                airspace.type_ = Some(read_text(reader, name)?);
            }
            QName(b"aixm:name") => {
                airspace.name = Some(read_text(reader, name)?);
            }
            QName(b"aixm:AirspaceVolume") => {
                let volume = parse_airspace_volume(reader)?;
                airspace.volumes.push(volume);
            }
            _ => (),
        }
    }

    Ok(airspace)
}

fn parse_airspace_volume<R: std::io::BufRead>(
    reader: &mut Reader<R>,
) -> Result<AirspaceVolume, Box<dyn std::error::Error>> {
    let mut volume = AirspaceVolume::default();

    while let Ok(node) = find_node(
        reader,
        vec![
            QName(b"aixm:upperLimit"),
            QName(b"aixm:upperLimitReference"),
            QName(b"aixm:lowerLimit"),
            QName(b"aixm:lowerLimitReference"),
            QName(b"gml:pos"),
            QName(b"gml:pointProperty"),
            QName(b"aixm:theAirspace"),
        ],
        Some(QName(b"aixm:AirspaceVolume")),
    ) {
        let Node { name, attributes } = node;
        match name {
            QName(b"aixm:upperLimit") => {
                volume.upper_limit = Some(read_text(reader, name)?);
            }
            QName(b"aixm:upperLimitReference") => {
                volume.upper_limit_reference = Some(read_text(reader, name)?);
            }
            QName(b"aixm:lowerLimit") => {
                volume.lower_limit = Some(read_text(reader, name)?);
            }
            QName(b"aixm:lowerLimitReference") => {
                volume.lower_limit_reference = Some(read_text(reader, name)?);
            }
            QName(b"gml:pos") => {
                let text = read_text(reader, name)?;
                let mut numbers = text.split_whitespace().filter_map(|x| x.parse::<f64>().ok());
                if let (Some(lat), Some(lon)) = (numbers.next(), numbers.next()) {
                    volume.polygon.push((lat, lon));
                }
            }
            QName(b"gml:pointProperty") => {
                if let Some(id) = attributes
                    .get("xlink:href")
                    .map(|s| s.strip_prefix("urn:uuid:").unwrap_or(s).to_string())
                {
                    volume.point_refs.push(id);
                }
            }
            QName(b"aixm:theAirspace") => {
                volume.component_airspace = attributes
                    .get("xlink:href")
                    .map(|s| s.strip_prefix("urn:uuid:").unwrap_or(s).to_string());
            }
            _ => (),
        }
    }

    Ok(volume)
}
