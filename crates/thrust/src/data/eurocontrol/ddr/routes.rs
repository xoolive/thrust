use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use super::navpoints::{find_navpoints_file, parse_navpoints_file, DdrNavPoint};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DdrRoutePoint {
    pub route: String,
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

pub fn parse_routes_dir<P: AsRef<Path>>(dir: P) -> Result<Vec<DdrRoutePoint>, Box<dyn std::error::Error>> {
    let dir = dir.as_ref();
    let route_file = find_routes_file(dir).ok_or("No AIRAC_*.routes file found")?;
    let nav_file = find_navpoints_file(dir).ok_or("No AIRAC_*.nnpt file found")?;
    let navpoints = parse_navpoints_file(nav_file)?;
    parse_routes_file(route_file, &navpoints)
}

pub fn parse_routes_file<P: AsRef<Path>>(
    path: P,
    navpoints: &[DdrNavPoint],
) -> Result<Vec<DdrRoutePoint>, Box<dyn std::error::Error>> {
    let nav_index: HashMap<String, (f64, f64)> = navpoints
        .iter()
        .map(|p| (p.name.to_uppercase(), (p.latitude, p.longitude)))
        .collect();

    let file = File::open(path)?;
    let reader = BufReader::new(file);
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
        let navaid = fields[5].trim().to_string();
        let point_type = fields[6].trim().to_string();
        let seq = fields[7].trim().parse::<i32>().unwrap_or(0);
        let coords = nav_index.get(&navaid.to_uppercase()).copied();

        result.push(DdrRoutePoint {
            route,
            seq,
            navaid,
            point_type,
            latitude: coords.map(|c| c.0),
            longitude: coords.map(|c| c.1),
        });
    }

    Ok(result)
}
