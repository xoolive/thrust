use crate::error::ThrustError;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor};
use std::path::{Path, PathBuf};

use super::navpoints::{find_navpoints_file, parse_navpoints_bytes, parse_navpoints_file, DdrNavPoint};
use super::{file_name_matches, read_first_zip_entry_bytes};

/// A sequential navigation point along an ATS route from DDR data.
///
/// Represents a single waypoint in a defined airway. Route points are ordered
/// by sequence number and collectively define the lateral path of an ATS route.
///
/// # Fields
/// - `route`: Route designator (e.g., "N100", "UN456")
/// - `seq`: Sequence number in route (1, 2, 3, ...)
/// - `navaid`: Navigation point identifier
/// - `point_type`: Classification (e.g., "NAVAID", "WAYPOINT", "AIRPORT")
/// - `latitude`: Optional point latitude in WGS84 decimal degrees
/// - `longitude`: Optional point longitude in WGS84 decimal degrees
///
/// # Example
/// ```ignore
/// let point = DdrRoutePoint {
///     route: "N100".to_string(),
///     seq: 1,
///     navaid: "APTIN".to_string(),
///     point_type: "WAYPOINT".to_string(),
///     latitude: Some(47.6213),
///     longitude: Some(-122.3007),
/// };
/// ```
///
/// # Note
/// Multiple [`DdrRoutePoint`]s with the same route designator should be
/// sorted by `seq` to reconstruct the complete route path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DdrRoutePoint {
    pub route: String,
    pub route_class: String,
    pub seq: i32,
    pub navaid: String,
    pub point_type: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

pub fn find_routes_file<P: AsRef<Path>>(dir: P) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.starts_with("AIRAC_") && n.ends_with(".routes"))
    })
}

pub fn parse_routes_dir<P: AsRef<Path>>(dir: P) -> Result<Vec<DdrRoutePoint>, ThrustError> {
    let dir = dir.as_ref();
    let route_file = find_routes_file(dir).ok_or("No AIRAC_*.routes file found")?;
    let nav_file = find_navpoints_file(dir).ok_or("No AIRAC_*.nnpt file found")?;
    let navpoints = parse_navpoints_file(nav_file)?;
    parse_routes_file(route_file, &navpoints)
}

pub fn parse_routes_path<P: AsRef<Path>>(path: P) -> Result<Vec<DdrRoutePoint>, ThrustError> {
    let path = path.as_ref();
    if path.is_dir() {
        return parse_routes_dir(path);
    }
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        return parse_routes_zip(path);
    }
    Err("DDR routes path must be a folder or a zip archive".into())
}

pub fn parse_routes_zip<P: AsRef<Path>>(zip_path: P) -> Result<Vec<DdrRoutePoint>, ThrustError> {
    let nav_bytes =
        read_first_zip_entry_bytes(&zip_path, |entry_name| file_name_matches(entry_name, "AIRAC_", ".nnpt"))?;
    let route_bytes = read_first_zip_entry_bytes(&zip_path, |entry_name| {
        file_name_matches(entry_name, "AIRAC_", ".routes")
    })?;
    let navpoints = parse_navpoints_bytes(&nav_bytes)?;
    parse_routes_bytes(&route_bytes, &navpoints)
}

pub fn parse_routes_file<P: AsRef<Path>>(
    path: P,
    navpoints: &[DdrNavPoint],
) -> Result<Vec<DdrRoutePoint>, ThrustError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    parse_routes_reader(reader, navpoints)
}

pub fn parse_routes_bytes(bytes: &[u8], navpoints: &[DdrNavPoint]) -> Result<Vec<DdrRoutePoint>, ThrustError> {
    parse_routes_reader(BufReader::new(Cursor::new(bytes)), navpoints)
}

fn parse_routes_reader<R: BufRead>(reader: R, navpoints: &[DdrNavPoint]) -> Result<Vec<DdrRoutePoint>, ThrustError> {
    let nav_index: HashMap<String, (f64, f64)> = navpoints
        .iter()
        .map(|p| (p.name.to_uppercase(), (p.latitude, p.longitude)))
        .collect();
    let mut result = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(';').collect();
        if fields.len() < 8 {
            continue;
        }
        let route = fields[1].trim().to_string();
        let route_class = fields[2].trim().to_string();
        let navaid = fields[5].trim().to_string();
        let point_type = fields[6].trim().to_string();
        let seq = fields[7].trim().parse::<i32>().unwrap_or(0);
        let coords = nav_index.get(&navaid.to_uppercase()).copied();

        result.push(DdrRoutePoint {
            route,
            route_class,
            seq,
            navaid,
            point_type,
            latitude: coords.map(|c| c.0),
            longitude: coords.map(|c| c.1),
        });
    }

    Ok(result)
}
