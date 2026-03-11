//! AIXM (Aeronautical Information Exchange Model) data parsers.
//!
//! This module provides parsers for various AIXM data types such as airports,
//! heliports, designated points, navaids, routes, route segments, STARs, and SIDs.
//!
//! The parsers are provided under an open source license and can be used to read
//! and process AIXM XML data files provided by EUROCONTROL B2B services under
//! a specific license agreement.

use std::collections::HashMap;

use quick_xml::{events::Event, name::QName, Reader};

use crate::error::ThrustError;

pub mod airport_heliport;
pub mod airspace;
pub mod arrival_leg;
pub mod dataset;
pub mod departure_leg;
pub mod designated_point;
pub mod navaid;
pub mod route;
pub mod route_segment;
pub mod standard_instrument_arrival;
pub mod standard_instrument_departure;

struct Node<'a> {
    name: QName<'a>,
    attributes: HashMap<String, String>,
}

fn find_node<'a, R: std::io::BufRead>(
    reader: &mut Reader<R>,
    lookup: Vec<QName<'a>>,
    end: Option<QName>,
) -> Result<Node<'a>, ThrustError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                for elt in lookup.iter() {
                    if e.name() == *elt {
                        let mut attributes = HashMap::new();

                        for attr in e.attributes().with_checks(false) {
                            let attr = attr?;
                            let key = std::str::from_utf8(attr.key.0)?;
                            attributes.insert(key.to_string(), attr.unescape_value()?.to_string());
                        }

                        return Ok(Node { name: *elt, attributes });
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                for elt in lookup.iter() {
                    if e.name() == *elt {
                        let mut attributes = HashMap::new();

                        for attr in e.attributes().with_checks(false) {
                            let attr = attr?;
                            let key = std::str::from_utf8(attr.key.0)?;
                            attributes.insert(key.to_string(), attr.unescape_value()?.to_string());
                        }

                        return Ok(Node { name: *elt, attributes });
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                if let Some(end) = end {
                    if e.name() == end {
                        break;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ThrustError::from(e)),
            _ => (),
        }
        buf.clear();
    }
    Err(ThrustError::ParseError("Node not found".to_string()))
}

fn read_text<R: std::io::BufRead>(reader: &mut Reader<R>, end: QName) -> Result<String, ThrustError> {
    let mut buf = Vec::new();
    let mut text = String::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(e)) => {
                let decoded = e
                    .decode()
                    .map_err(|_| ThrustError::ParseError("Invalid XML encoding".to_string()))?;
                text.push_str(&decoded);
            }
            Ok(Event::End(e)) if e.name() == end => break,
            Ok(Event::Eof) => break,
            Err(e) => return Err(ThrustError::from(e)),
            _ => (),
        }
        buf.clear();
    }
    Ok(text)
}

pub fn read_attribute<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    attr_name: QName,
) -> Result<Option<String>, ThrustError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                for attr in e.attributes().with_checks(false) {
                    let attr = attr?;
                    if attr.key == attr_name {
                        return Ok(Some(attr.unescape_value()?.to_string()));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ThrustError::from(e)),
            _ => (),
        }
        buf.clear();
    }
    Ok(None)
}
