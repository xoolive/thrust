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
 * A route as defined in AIXM.
 *
 * A route name consists of a prefix (optional), a second letter, a number, and
 * a multiple identifier (optional).
 *
 * For example, "UN123" has:
 *   prefix: "U", second_letter: "N", number: "123", multiple_identifier: None
 *
 * Another example, "N456B" has:
 *   prefix: None, second_letter: "N", number: "456", multiple_identifier: "B"
 *
 */

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Route {
    #[serde(skip)]
    pub identifier: String,
    /// The prefix of the route, if any (must be "U" if any)
    pub prefix: Option<String>,
    /// The second letter of the route
    pub second_letter: Option<String>,
    /// The number of the route
    pub number: Option<String>,
    /// The multiple identifier of the route, if any
    pub multiple_identifier: Option<String>,
}

/**
 * Parse route data from a ZIP file containing AIXM data.
 */
pub fn parse_route_zip_file<P: AsRef<Path>>(path: P) -> Result<HashMap<String, Route>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut routes = HashMap::new();

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.name().ends_with(".BASELINE") {
            let mut reader = Reader::from_reader(BufReader::new(file));

            while let Ok(_node) = find_node(&mut reader, vec![QName(b"aixm:Route")], None) {
                let route = parse_route(&mut reader)?;
                routes.insert(route.identifier.clone(), route);
            }
        }
    }

    Ok(routes)
}

fn parse_route<R: std::io::BufRead>(reader: &mut Reader<R>) -> Result<Route, Box<dyn std::error::Error>> {
    let mut route = Route::default();

    while let Ok(node) = find_node(
        reader,
        vec![
            QName(b"gml:identifier"),
            QName(b"aixm:designatorPrefix"),
            QName(b"aixm:designatorSecondLetter"),
            QName(b"aixm:designatorNumber"),
            QName(b"aixm:multipleIdentifier"),
        ],
        Some(QName(b"aixm:Route")),
    ) {
        let Node { name, .. } = node;
        match name {
            QName(b"gml:identifier") => {
                route.identifier = read_text(reader, name)?;
            }
            QName(b"aixm:designatorPrefix") => {
                route.prefix = Some(read_text(reader, name)?);
            }
            QName(b"aixm:designatorSecondLetter") => {
                route.second_letter = Some(read_text(reader, name)?);
            }
            QName(b"aixm:designatorNumber") => {
                route.number = Some(read_text(reader, name)?);
            }
            QName(b"aixm:multipleIdentifier") => {
                route.multiple_identifier = Some(read_text(reader, name)?);
            }
            _ => (),
        }
    }
    Ok(route)
}
