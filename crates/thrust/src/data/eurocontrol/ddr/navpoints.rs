use crate::error::ThrustError;

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor};
use std::path::{Path, PathBuf};

use super::{file_name_matches, read_first_zip_entry_bytes};

/// A navigation point from the EUROCONTROL DDR (Data Display Requirements) database.
///
/// Represents waypoints, radio navigation aids, and other significant points
/// extracted from Eurocontrol DDR files. Contains essential positioning and
/// classification data used for flight planning and route validation.
///
/// # Fields
/// - `name`: Point designator/identifier (e.g., "APTIN", "SEA", "JFK")
/// - `point_type`: Classification (e.g., "WAYPOINT", "NAVAID", "AIRPORT", "HOLDING POINT")
/// - `latitude`: Location latitude in WGS84 decimal degrees
/// - `longitude`: Location longitude in WGS84 decimal degrees
/// - `description`: Optional additional information or location name
///
/// # Example
/// ```ignore
/// let point = DdrNavPoint {
///     name: "APTIN".to_string(),
///     point_type: "WAYPOINT".to_string(),
///     latitude: 47.6213,
///     longitude: -122.3007,
///     description: Some("Approach waypoint".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DdrNavPoint {
    pub name: String,
    pub point_type: String,
    pub latitude: f64,
    pub longitude: f64,
    pub description: Option<String>,
}

pub fn find_navpoints_file<P: AsRef<Path>>(dir: P) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.starts_with("AIRAC_") && n.ends_with(".nnpt"))
    })
}

pub fn parse_navpoints_dir<P: AsRef<Path>>(dir: P) -> Result<Vec<DdrNavPoint>, ThrustError> {
    let file = find_navpoints_file(dir).ok_or("No AIRAC_*.nnpt file found")?;
    parse_navpoints_file(file)
}

pub fn parse_navpoints_path<P: AsRef<Path>>(path: P) -> Result<Vec<DdrNavPoint>, ThrustError> {
    let path = path.as_ref();
    if path.is_dir() {
        return parse_navpoints_dir(path);
    }
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        return parse_navpoints_zip(path);
    }
    parse_navpoints_file(path)
}

pub fn parse_navpoints_zip<P: AsRef<Path>>(zip_path: P) -> Result<Vec<DdrNavPoint>, ThrustError> {
    let bytes = read_first_zip_entry_bytes(zip_path, |entry_name| file_name_matches(entry_name, "AIRAC_", ".nnpt"))?;
    parse_navpoints_bytes(&bytes)
}

pub fn parse_navpoints_file<P: AsRef<Path>>(path: P) -> Result<Vec<DdrNavPoint>, ThrustError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    parse_navpoints_reader(reader)
}

pub fn parse_navpoints_bytes(bytes: &[u8]) -> Result<Vec<DdrNavPoint>, ThrustError> {
    parse_navpoints_reader(BufReader::new(Cursor::new(bytes)))
}

fn parse_navpoints_reader<R: BufRead>(reader: R) -> Result<Vec<DdrNavPoint>, ThrustError> {
    let mut points = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(';').collect();
        if fields.len() < 5 {
            continue;
        }
        let lat = fields[2].trim().parse::<f64>().ok();
        let lon = fields[3].trim().parse::<f64>().ok();
        if let (Some(latitude), Some(longitude)) = (lat, lon) {
            points.push(DdrNavPoint {
                name: fields[0].trim().to_string(),
                point_type: fields[1].trim().to_string(),
                latitude,
                longitude,
                description: fields
                    .get(4)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && s != "_"),
            });
        }
    }

    Ok(points)
}
