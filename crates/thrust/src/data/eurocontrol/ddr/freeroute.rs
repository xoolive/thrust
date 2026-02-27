use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use super::airspaces::{find_file_with_prefix_suffix, parse_are_file, parse_sls_file, DdrSectorLayer};
use super::navpoints::DdrNavPoint;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DdrFreeRoutePoint {
    pub fra: String,
    pub point_type: String,
    pub name: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DdrFreeRouteData {
    pub areas: Vec<DdrSectorLayer>,
    pub points: Vec<DdrFreeRoutePoint>,
}

pub fn parse_freeroute_dir<P: AsRef<Path>>(
    dir: P,
    navpoints: &[DdrNavPoint],
) -> Result<DdrFreeRouteData, Box<dyn std::error::Error>> {
    let dir = dir.as_ref();
    let are = find_file_with_prefix_suffix(dir, "Free_Route_", ".are").ok_or("No Free_Route_*.are file")?;
    let sls = find_file_with_prefix_suffix(dir, "Free_Route_", ".sls").ok_or("No Free_Route_*.sls file")?;
    let frp = find_file_with_prefix_suffix(dir, "Free_Route_", ".frp").ok_or("No Free_Route_*.frp file")?;

    let polygons = parse_are_file(are)?;
    let areas = parse_sls_file(sls, &polygons)?;
    let points = parse_frp_file(frp, navpoints)?;

    Ok(DdrFreeRouteData { areas, points })
}

pub fn parse_frp_file<P: AsRef<Path>>(
    path: P,
    navpoints: &[DdrNavPoint],
) -> Result<Vec<DdrFreeRoutePoint>, Box<dyn std::error::Error>> {
    let nav_index: HashMap<String, (f64, f64)> = navpoints
        .iter()
        .map(|p| (p.name.to_uppercase(), (p.latitude, p.longitude)))
        .collect();

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut points = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 3 {
            continue;
        }
        let fra = tokens[0].to_string();
        let point_type = tokens[1].to_string();
        let name = tokens[2].to_string();

        let parsed_coords = parse_fra_coordinate(&name);
        let coords = parsed_coords.or_else(|| nav_index.get(&name.to_uppercase()).copied());

        points.push(DdrFreeRoutePoint {
            fra,
            point_type,
            name,
            latitude: coords.map(|c| c.0),
            longitude: coords.map(|c| c.1),
        });
    }

    Ok(points)
}

fn parse_fra_coordinate(token: &str) -> Option<(f64, f64)> {
    let token = token.trim();
    let ns_pos = token.find('N').or_else(|| token.find('S'))?;
    let ew_pos = token.find('E').or_else(|| token.find('W'))?;
    if ns_pos < 4 || ew_pos <= ns_pos + 1 {
        return None;
    }

    let lat_raw = &token[..ns_pos];
    let lon_raw = &token[ns_pos + 1..ew_pos];
    if !(lat_raw.len() == 4 || lat_raw.len() == 6) || !(lon_raw.len() == 5 || lon_raw.len() == 7) {
        return None;
    }

    let lat_sign = if &token[ns_pos..=ns_pos] == "S" { -1.0 } else { 1.0 };
    let lon_sign = if &token[ew_pos..=ew_pos] == "W" { -1.0 } else { 1.0 };

    let lat_pad = format!("{lat_raw:0<6}");
    let lon_pad = format!("{lon_raw:0<7}");

    let lat = lat_pad[..2].parse::<f64>().ok()?
        + lat_pad[2..4].parse::<f64>().ok()? / 60.0
        + lat_pad[4..].parse::<f64>().ok()? / 3600.0;
    let lon = lon_pad[..3].parse::<f64>().ok()?
        + lon_pad[3..5].parse::<f64>().ok()? / 60.0
        + lon_pad[5..].parse::<f64>().ok()? / 3600.0;
    Some((lat * lat_sign, lon * lon_sign))
}
