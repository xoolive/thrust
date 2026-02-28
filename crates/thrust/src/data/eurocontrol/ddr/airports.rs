use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor};
use std::path::Path;

use super::{file_name_matches, read_first_zip_entry_bytes};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DdrAirport {
    pub code: String,
    pub latitude: f64,
    pub longitude: f64,
}

pub fn parse_airports_path<P: AsRef<Path>>(path: P) -> Result<Vec<DdrAirport>, Box<dyn std::error::Error>> {
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

pub fn parse_airports_dir<P: AsRef<Path>>(dir: P) -> Result<Vec<DdrAirport>, Box<dyn std::error::Error>> {
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

pub fn parse_airports_zip<P: AsRef<Path>>(zip_path: P) -> Result<Vec<DdrAirport>, Box<dyn std::error::Error>> {
    let bytes = read_first_zip_entry_bytes(zip_path, |entry_name| {
        file_name_matches(entry_name, "VST_", "_Airports.arp")
    })?;
    parse_airports_bytes(&bytes)
}

pub fn parse_airports_file<P: AsRef<Path>>(path: P) -> Result<Vec<DdrAirport>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    parse_airports_reader(BufReader::new(file))
}

pub fn parse_airports_bytes(bytes: &[u8]) -> Result<Vec<DdrAirport>, Box<dyn std::error::Error>> {
    parse_airports_reader(BufReader::new(Cursor::new(bytes)))
}

fn parse_airports_reader<R: BufRead>(reader: R) -> Result<Vec<DdrAirport>, Box<dyn std::error::Error>> {
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

        airports.push(DdrAirport {
            code,
            latitude: lat_raw / 100.0,
            longitude: lon_raw / 100.0,
        });
    }
    Ok(airports)
}
