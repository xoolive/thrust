use crate::error::ThrustError;

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor};
use std::path::Path;

use super::{file_name_matches, read_first_zip_entry_bytes};

/// An airport or heliport location from the Eurocontrol DDR (Data Display Requirements) database.
///
/// Represents simplified airport data extracted from EUROCONTROL DDR files,
/// containing only ICAO code and WGS84 coordinates for basic geographic queries.
///
/// # Fields
/// - `code`: ICAO airport identifier (e.g., "KSEA", "EGLL")
/// - `latitude`: Runway/field elevation reference point latitude in decimal degrees
/// - `longitude`: Runway/field elevation reference point longitude in decimal degrees
///
/// # Example
/// ```ignore
/// let airport = DdrAirport {
///     code: "KSEA".to_string(),
///     latitude: 47.4502,
///     longitude: -122.3088,
/// };
/// ```
///
/// # Note
/// For detailed airport information (elevation, IATA codes, names, etc.),
/// use [`AirportHeliport`](crate::data::eurocontrol::aixm::airport_heliport::AirportHeliport) from AIXM data instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DdrAirport {
    pub code: String,
    pub latitude: f64,
    pub longitude: f64,
}

fn decode_ddr_coords(lat_raw: f64, lon_raw: f64) -> Option<(f64, f64)> {
    if lat_raw.abs() <= 90.0 && lon_raw.abs() <= 180.0 {
        return Some((lat_raw, lon_raw));
    }

    let lat_minutes = lat_raw / 60.0;
    let lon_minutes = lon_raw / 60.0;
    if lat_minutes.abs() <= 90.0 && lon_minutes.abs() <= 180.0 {
        return Some((lat_minutes, lon_minutes));
    }

    let lat_scaled = lat_raw / 600_000.0;
    let lon_scaled = lon_raw / 600_000.0;
    if lat_scaled.abs() <= 90.0 && lon_scaled.abs() <= 180.0 {
        return Some((lat_scaled, lon_scaled));
    }

    None
}

pub fn parse_airports_path<P: AsRef<Path>>(path: P) -> Result<Vec<DdrAirport>, ThrustError> {
    let path = path.as_ref();
    if path.is_dir() {
        return parse_airports_dir(path);
    }
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        return parse_airports_zip(path);
    }
    Err("DDR airports path must be a folder or a zip archive".into())
}

pub fn parse_airports_dir<P: AsRef<Path>>(dir: P) -> Result<Vec<DdrAirport>, ThrustError> {
    let file = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.starts_with("VST_") && n.ends_with("_Airports.arp"))
        })
        .ok_or("Unable to find VST_*_Airports.arp")?;
    parse_airports_file(file)
}

pub fn parse_airports_zip<P: AsRef<Path>>(zip_path: P) -> Result<Vec<DdrAirport>, ThrustError> {
    let bytes = read_first_zip_entry_bytes(zip_path, |entry_name| {
        file_name_matches(entry_name, "VST_", "_Airports.arp")
    })?;
    parse_airports_bytes(&bytes)
}

pub fn parse_airports_file<P: AsRef<Path>>(path: P) -> Result<Vec<DdrAirport>, ThrustError> {
    let file = File::open(path)?;
    parse_airports_reader(BufReader::new(file))
}

pub fn parse_airports_bytes(bytes: &[u8]) -> Result<Vec<DdrAirport>, ThrustError> {
    parse_airports_reader(BufReader::new(Cursor::new(bytes)))
}

fn parse_airports_reader<R: BufRead>(reader: R) -> Result<Vec<DdrAirport>, ThrustError> {
    let mut airports = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 3 {
            continue;
        }
        let code = parts[0].trim().to_uppercase();
        if code.len() != 4 {
            continue;
        }
        let lat_raw = parts[1].parse::<f64>().ok();
        let lon_raw = parts[2].parse::<f64>().ok();
        let (lat_raw, lon_raw) = match (lat_raw, lon_raw) {
            (Some(a), Some(b)) => (a, b),
            _ => continue,
        };

        let Some((latitude, longitude)) = decode_ddr_coords(lat_raw, lon_raw) else {
            continue;
        };

        airports.push(DdrAirport {
            code,
            latitude,
            longitude,
        });
    }
    Ok(airports)
}

#[cfg(test)]
mod tests {
    use super::parse_airports_bytes;

    #[test]
    fn parse_lfbo_coordinates_from_ddr_arp() {
        let sample = b"LFBO 2618.100000 82.066667\n";
        let airports = parse_airports_bytes(sample).expect("failed to parse sample airports");
        let lfbo = airports.iter().find(|a| a.code == "LFBO").expect("LFBO not found");

        assert!((lfbo.latitude - 43.635).abs() < 1e-9);
        assert!((lfbo.longitude - 1.3677777833333334).abs() < 1e-9);
    }
}
