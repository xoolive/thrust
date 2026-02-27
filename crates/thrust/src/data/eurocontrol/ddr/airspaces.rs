use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DdrPolygon {
    pub name: String,
    pub coordinates: Vec<(f64, f64)>, // (lon, lat)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DdrSectorLayer {
    pub designator: String,
    pub polygon_name: String,
    pub lower: f64,
    pub upper: f64,
    pub coordinates: Vec<(f64, f64)>,
}

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

pub fn parse_are_file<P: AsRef<Path>>(path: P) -> Result<HashMap<String, DdrPolygon>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

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
) -> Result<Vec<DdrSectorLayer>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
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

pub fn parse_spc_file<P: AsRef<Path>>(path: P) -> Result<Vec<DdrCollapsedSector>, Box<dyn std::error::Error>> {
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
