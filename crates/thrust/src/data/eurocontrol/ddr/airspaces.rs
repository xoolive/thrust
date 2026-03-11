use crate::error::ThrustError;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor};
use std::path::{Path, PathBuf};

use super::{file_name_matches, read_first_zip_entry_bytes};

/// A geographic polygon defined by boundary coordinates (longitude, latitude pairs).
///
/// Polygons are used to represent airspace boundaries in EUROCONTROL DDR (Demand Data Repository) data.
/// Coordinates are stored as (longitude, latitude) tuples and define a closed boundary.
///
/// # Fields
/// - `name`: Identifier or name of the polygon (e.g., "UIR_EGTT")
/// - `coordinates`: Ordered sequence of (lon, lat) pairs forming the boundary
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DdrPolygon {
    pub name: String,
    pub coordinates: Vec<(f64, f64)>, // (lon, lat)
}

/// A vertical section of an airspace sector with altitude bounds and boundary geometry.
///
/// Sectors in EUROCONTROL DDR data are divided into layers (vertical slices) to capture
/// altitude-dependent airspace properties. Each layer represents a portion of an ATC sector
/// between two flight levels.
///
/// # Fields
/// - `designator`: Sector identifier (e.g., "UGTW_S01" for UK London South sector 1)
/// - `polygon_name`: Reference to the geographic boundary polygon
/// - `lower`: Lower flight level bound (feet, mean sea level)
/// - `upper`: Upper flight level bound (feet, mean sea level)
/// - `coordinates`: Boundary coordinates (copied from polygon for convenience)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DdrSectorLayer {
    pub designator: String,
    pub polygon_name: String,
    pub lower: f64,
    pub upper: f64,
    pub coordinates: Vec<(f64, f64)>,
}

/// A simplified sector definition combining multiple vertical layers.
///
/// This represents a single ATC sector (control zone) in its entirety, aggregating
/// all altitude layers. Used for high-level airspace summaries.
///
/// # Fields
/// - `designator`: Sector identifier (e.g., "UGTW_S01")
/// - `component`: Organizational component (e.g., "LONDON", "SCOTTISH")
/// - `name`: Full sector name (optional)
/// - `sector_type`: Classification of the sector (e.g., "TMA", "UIR", "FIR")
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DdrCollapsedSector {
    pub designator: String,
    pub component: String,
    pub name: Option<String>,
    pub sector_type: Option<String>,
}

pub fn find_file_with_prefix_suffix<P: AsRef<Path>>(dir: P, prefix: &str, suffix: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.starts_with(prefix) && n.ends_with(suffix))
    })
}

pub fn parse_are_file<P: AsRef<Path>>(path: P) -> Result<HashMap<String, DdrPolygon>, ThrustError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    parse_are_reader(reader)
}

pub fn parse_are_bytes(bytes: &[u8]) -> Result<HashMap<String, DdrPolygon>, ThrustError> {
    parse_are_reader(BufReader::new(Cursor::new(bytes)))
}

fn parse_are_reader<R: BufRead>(reader: R) -> Result<HashMap<String, DdrPolygon>, ThrustError> {
    let mut polygons = HashMap::new();
    let mut expected_points = 0usize;
    let mut current_name = String::new();
    let mut current_coords: Vec<(f64, f64)> = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if expected_points == 0 {
            if !current_name.is_empty() {
                polygons.insert(
                    current_name.clone(),
                    DdrPolygon {
                        name: current_name.clone(),
                        coordinates: current_coords.clone(),
                    },
                );
            }
            current_coords.clear();
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let (Some(n), Some(name)) = (parts.first(), parts.last()) {
                expected_points = n.parse::<usize>().unwrap_or(0);
                current_name = (*name).to_string();
            }
        } else {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let lat = parts[0].parse::<f64>().unwrap_or(0.0) / 60.0;
                let lon = parts[1].parse::<f64>().unwrap_or(0.0) / 60.0;
                current_coords.push((lon, lat));
            }
            expected_points = expected_points.saturating_sub(1);
        }
    }

    if !current_name.is_empty() && !current_coords.is_empty() {
        polygons.insert(
            current_name.clone(),
            DdrPolygon {
                name: current_name,
                coordinates: current_coords,
            },
        );
    }

    Ok(polygons)
}

pub fn parse_sls_file<P: AsRef<Path>>(
    path: P,
    polygons: &HashMap<String, DdrPolygon>,
) -> Result<Vec<DdrSectorLayer>, ThrustError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    parse_sls_reader(reader, polygons)
}

pub fn parse_sls_bytes(
    bytes: &[u8],
    polygons: &HashMap<String, DdrPolygon>,
) -> Result<Vec<DdrSectorLayer>, ThrustError> {
    parse_sls_reader(BufReader::new(Cursor::new(bytes)), polygons)
}

fn parse_sls_reader<R: BufRead>(
    reader: R,
    polygons: &HashMap<String, DdrPolygon>,
) -> Result<Vec<DdrSectorLayer>, ThrustError> {
    let mut layers = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }
        let designator = parts[0].to_string();
        let polygon_name = parts[2].to_string();
        let lower = parts[3].parse::<f64>().unwrap_or(0.0);
        let upper = parts[4].parse::<f64>().unwrap_or(0.0);
        let coordinates = polygons
            .get(&polygon_name)
            .map(|p| p.coordinates.clone())
            .unwrap_or_default();

        layers.push(DdrSectorLayer {
            designator,
            polygon_name,
            lower,
            upper,
            coordinates,
        });
    }
    Ok(layers)
}

fn parse_layers_from_dir<P: AsRef<Path>>(dir: P, prefix: &str) -> Result<Vec<DdrSectorLayer>, ThrustError> {
    let dir = dir.as_ref();
    let are =
        find_file_with_prefix_suffix(dir, prefix, ".are").ok_or_else(|| format!("Unable to find {prefix}*.are"))?;
    let sls =
        find_file_with_prefix_suffix(dir, prefix, ".sls").ok_or_else(|| format!("Unable to find {prefix}*.sls"))?;
    let polygons = parse_are_file(are)?;
    parse_sls_file(sls, &polygons)
}

fn parse_layers_from_zip<P: AsRef<Path>>(zip_path: P, prefix: &str) -> Result<Vec<DdrSectorLayer>, ThrustError> {
    let are_bytes = read_first_zip_entry_bytes(&zip_path, |entry_name| file_name_matches(entry_name, prefix, ".are"))?;
    let sls_bytes = read_first_zip_entry_bytes(&zip_path, |entry_name| file_name_matches(entry_name, prefix, ".sls"))?;
    let polygons = parse_are_bytes(&are_bytes)?;
    parse_sls_bytes(&sls_bytes, &polygons)
}

pub fn parse_sector_layers_path<P: AsRef<Path>>(path: P) -> Result<Vec<DdrSectorLayer>, ThrustError> {
    let path = path.as_ref();
    if path.is_dir() {
        return parse_layers_from_dir(path, "Sectors_");
    }
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        return parse_layers_from_zip(path, "Sectors_");
    }
    Err("DDR sector path must be a folder or a zip archive".into())
}

pub fn parse_fra_layers_path<P: AsRef<Path>>(path: P) -> Result<Vec<DdrSectorLayer>, ThrustError> {
    let path = path.as_ref();
    if path.is_dir() {
        return parse_layers_from_dir(path, "Free_Route_");
    }
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        return parse_layers_from_zip(path, "Free_Route_");
    }
    Err("DDR FRA path must be a folder or a zip archive".into())
}

pub fn parse_spc_file<P: AsRef<Path>>(path: P) -> Result<Vec<DdrCollapsedSector>, ThrustError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut list = Vec::new();

    let mut current_name = String::new();
    let mut current_type: Option<String> = None;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split(';').collect();
        if parts.is_empty() {
            continue;
        }
        match parts[0] {
            "A" => {
                if parts.len() >= 4 {
                    current_name = parts[1].to_string();
                    current_type = parts.get(3).map(|s| s.to_string());
                }
            }
            "S" => {
                if parts.len() >= 3 {
                    let component = parts[1].to_string();
                    list.push(DdrCollapsedSector {
                        designator: current_name.clone(),
                        component: component.clone(),
                        name: Some(parts[2].to_string()),
                        sector_type: current_type.clone(),
                    });
                }
            }
            _ => {}
        }
    }

    Ok(list)
}
