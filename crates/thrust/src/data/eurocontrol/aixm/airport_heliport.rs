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
 * An airport or heliport as defined in AIXM.
 */
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AirportHeliport {
    /// Unique identifier
    #[serde(skip)]
    pub identifier: String,
    /// Latitude in decimal degrees
    pub latitude: f64,
    /// Longitude in decimal degrees
    pub longitude: f64,
    /// Altitude in feet
    pub altitude: f64,
    /// IATA code, if available
    pub iata: Option<String>,
    /// ICAO code
    pub icao: String,
    /// Name of the airport/heliport
    pub name: String,
    /// City served by the airport/heliport
    pub city: Option<String>,
    /// Type of airport/heliport
    pub r#type: String,
}

/// Parse airport/heliport data from a ZIP file containing AIXM data.
pub fn parse_airport_heliport_zip_file<P: AsRef<Path>>(
    path: P,
) -> Result<HashMap<String, AirportHeliport>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut airports = HashMap::new();

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.name().ends_with(".BASELINE") {
            let mut reader = Reader::from_reader(BufReader::new(file));
            while let Ok(_node) = find_node(&mut reader, vec![QName(b"aixm:AirportHeliport")], None) {
                let airport = parse_airport_heliport(&mut reader)?;
                airports.insert(airport.identifier.clone(), airport);
            }
        }
    }
    Ok(airports)
}

fn parse_airport_heliport<R: std::io::BufRead>(
    reader: &mut Reader<R>,
) -> Result<AirportHeliport, Box<dyn std::error::Error>> {
    let mut airport = AirportHeliport::default();

    while let Ok(node) = find_node(
        reader,
        vec![
            QName(b"gml:identifier"),
            QName(b"aixm:locationIndicatorICAO"),
            QName(b"aixm:designatorIATA"),
            QName(b"aixm:name"),
            QName(b"aixm:servedCity"),
            QName(b"aixm:controlType"),
            QName(b"aixm:ElevatedPoint"),
        ],
        Some(QName(b"aixm:AirportHeliport")),
    ) {
        let Node { name, .. } = node;
        match name {
            QName(b"gml:identifier") => {
                airport.identifier = read_text(reader, name)?;
            }
            QName(b"aixm:locationIndicatorICAO") => {
                airport.icao = read_text(reader, name)?;
            }
            QName(b"aixm:designatorIATA") => {
                airport.iata = Some(read_text(reader, name)?);
            }
            QName(b"aixm:name") => {
                airport.name = read_text(reader, name)?;
            }

            QName(b"aixm:servedCity") => {
                find_node(reader, vec![QName(b"aixm:City")], Some(name))?;
                find_node(reader, vec![QName(b"aixm:name")], Some(QName(b"aixm:City")))?;
                airport.city = Some(read_text(reader, QName(b"aixm:name"))?);
            }
            QName(b"aixm:controlType") => {
                airport.r#type = read_text(reader, name)?;
            }
            QName(b"aixm:ElevatedPoint") => {
                while let Ok(node) = find_node(reader, vec![QName(b"gml:pos"), QName(b"aixm:elevation")], Some(name)) {
                    let Node { name, .. } = node;
                    match name {
                        QName(b"gml:pos") => {
                            let coords: Vec<f64> = read_text(reader, name)?
                                .split_whitespace()
                                .map(|s| s.parse().unwrap())
                                .collect();
                            airport.latitude = coords[0];
                            airport.longitude = coords[1];
                        }
                        QName(b"aixm:elevation") => {
                            airport.altitude = read_text(reader, name)?.parse()?;
                        }
                        _ => (),
                    }
                }
            }
            _ => (),
        }
    }

    Ok(airport)
}
