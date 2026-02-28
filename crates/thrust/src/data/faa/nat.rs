use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::data::faa::nasr::NasrPoint;

#[cfg(feature = "net")]
const FAA_NAT_URL: &str = "https://notams.aim.faa.gov/nat.html";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NatDirection {
    East,
    West,
    Both,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NatPoint {
    pub token: String,
    pub name: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NatTrack {
    pub track_id: String,
    pub route_points: Vec<NatPoint>,
    pub east_levels: Vec<u16>,
    pub west_levels: Vec<u16>,
    pub nar_routes: Vec<String>,
    pub validity: Option<String>,
    pub source_center: Option<String>,
}

impl NatTrack {
    pub fn direction(&self) -> NatDirection {
        match (self.east_levels.is_empty(), self.west_levels.is_empty()) {
            (false, true) => NatDirection::East,
            (true, false) => NatDirection::West,
            (false, false) => NatDirection::Both,
            (true, true) => NatDirection::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NatBulletin {
    pub tracks: Vec<NatTrack>,
    pub tmi: Option<String>,
    pub updated_at: Option<String>,
}

pub fn fetch_nat_bulletin() -> Result<NatBulletin, Box<dyn std::error::Error>> {
    #[cfg(not(feature = "net"))]
    {
        Err("FAA NAT network fetch is disabled; enable feature 'net'".into())
    }

    #[cfg(feature = "net")]
    {
        let text = reqwest::blocking::Client::new()
            .get(FAA_NAT_URL)
            .timeout(std::time::Duration::from_secs(60))
            .send()?
            .error_for_status()?
            .text()?;
        Ok(parse_nat_bulletin(&text))
    }
}

pub fn parse_nat_bulletin(raw: &str) -> NatBulletin {
    let mut bulletin = NatBulletin::default();

    let normalized = normalize_text(raw);
    bulletin.updated_at = extract_updated_at(&normalized);
    bulletin.tmi = extract_tmi(&normalized);

    let mut current_validity: Option<String> = None;
    let mut current_center: Option<String> = None;
    let mut current_track: Option<NatTrack> = None;

    for line in normalized.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        if line.ends_with("ZOZX") || line.ends_with("ZQZX") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                current_center = Some(parts[1].to_string());
            }
            continue;
        }

        if line.contains(" TO ") && line.contains('Z') && line.contains('/') {
            current_validity = Some(line.to_string());
            continue;
        }

        if let Some(track) = parse_track_start_line(line) {
            if let Some(prev) = current_track.take() {
                bulletin.tracks.push(prev);
            }
            let mut track = track;
            track.validity = current_validity.clone();
            track.source_center = current_center.clone();
            current_track = Some(track);
            continue;
        }

        if let Some(track) = current_track.as_mut() {
            if let Some(levels) = line.strip_prefix("EAST LVLS") {
                track.east_levels = parse_levels(levels);
                continue;
            }
            if let Some(levels) = line.strip_prefix("WEST LVLS") {
                track.west_levels = parse_levels(levels);
                continue;
            }
            if let Some(nar) = line.strip_prefix("NAR") {
                track.nar_routes = parse_nar_routes(nar);
                continue;
            }
        }
    }

    if let Some(prev) = current_track.take() {
        bulletin.tracks.push(prev);
    }

    bulletin
}

pub fn resolve_named_points_with_nasr(bulletin: &mut NatBulletin, points: &[NasrPoint]) -> usize {
    let lookup = build_point_lookup(points);
    let mut resolved = 0usize;

    for track in &mut bulletin.tracks {
        for point in &mut track.route_points {
            if point.latitude.is_some() && point.longitude.is_some() {
                continue;
            }
            let key = point.token.to_uppercase();
            if let Some((lat, lon, name)) = lookup.get(&key) {
                point.latitude = Some(*lat);
                point.longitude = Some(*lon);
                if point.name.is_none() {
                    point.name = Some(name.clone());
                }
                resolved += 1;
            }
        }
    }

    resolved
}

fn build_point_lookup(points: &[NasrPoint]) -> HashMap<String, (f64, f64, String)> {
    let mut lookup = HashMap::new();
    for p in points {
        if p.latitude == 0.0 && p.longitude == 0.0 {
            continue;
        }

        let canonical_name = p.name.clone().unwrap_or_else(|| p.identifier.clone());
        let val = (p.latitude, p.longitude, canonical_name);

        lookup.entry(p.identifier.to_uppercase()).or_insert(val.clone());
        if let Some(name) = &p.name {
            lookup.entry(name.to_uppercase()).or_insert(val.clone());
        }

        let base = p.identifier.split(':').next().unwrap_or(&p.identifier).to_uppercase();
        lookup.entry(base).or_insert(val);
    }
    lookup
}

fn normalize_text(raw: &str) -> String {
    raw.replace(['\u{2}', '\u{3}', '\u{b}', '\r'], "\n")
}

fn extract_updated_at(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let marker = "Last updated at";
        if let Some(i) = line.find(marker) {
            let tail = line[i + marker.len()..].trim();
            let clean = tail.split('<').next().unwrap_or(tail).trim();
            if clean.is_empty() {
                None
            } else {
                Some(clean.to_string())
            }
        } else {
            None
        }
    })
}

fn extract_tmi(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        if let Some(idx) = line.find("TMI IS") {
            let tail = &line[idx + 6..];
            tail.split(|c: char| !c.is_ascii_alphanumeric())
                .find(|tok| !tok.is_empty())
                .map(|s| s.to_string())
        } else {
            None
        }
    })
}

fn parse_track_start_line(line: &str) -> Option<NatTrack> {
    let mut parts = line.split_whitespace();
    let id = parts.next()?;
    if id.len() != 1 || !id.chars().all(|c| c.is_ascii_uppercase()) {
        return None;
    }

    let route_points = parts
        .map(|p| p.trim_matches('-'))
        .filter(|p| !p.is_empty())
        .map(parse_nat_point)
        .collect::<Vec<_>>();

    if route_points.len() < 2 {
        return None;
    }

    Some(NatTrack {
        track_id: id.to_string(),
        route_points,
        ..Default::default()
    })
}

fn parse_nat_point(token: &str) -> NatPoint {
    let token = token.trim().to_string();
    if let Some((lat, lon)) = parse_coordinate_token(&token) {
        NatPoint {
            token,
            name: None,
            latitude: Some(lat),
            longitude: Some(lon),
        }
    } else {
        NatPoint {
            token: token.clone(),
            name: Some(token),
            latitude: None,
            longitude: None,
        }
    }
}

fn parse_coordinate_token(token: &str) -> Option<(f64, f64)> {
    // NAT shorthand like 50/50 means 50N 50W
    if let Some((lat_s, lon_s)) = token.split_once('/') {
        let lat = lat_s.parse::<f64>().ok()?;
        let lon = lon_s.parse::<f64>().ok()?;
        return Some((lat, -lon));
    }

    // 50N080W or 56N030W (degrees)
    if let Some((lat, lon)) = parse_ddn_dddw(token) {
        return Some((lat, lon));
    }

    // 5130N07000W or 5530N04000W (degrees+minutes)
    if let Some((lat, lon)) = parse_ddmmn_dddmmw(token) {
        return Some((lat, lon));
    }

    None
}

fn parse_ddn_dddw(token: &str) -> Option<(f64, f64)> {
    let b = token.as_bytes();
    if b.len() < 7 || b.len() > 9 {
        return None;
    }
    let n_pos = token.find('N').or_else(|| token.find('S'))?;
    let w_pos = token.find('W').or_else(|| token.find('E'))?;
    if n_pos < 2 || w_pos <= n_pos + 1 {
        return None;
    }
    let lat_deg = token[..n_pos].parse::<f64>().ok()?;
    let lon_deg = token[n_pos + 1..w_pos].parse::<f64>().ok()?;
    let lat = if &token[n_pos..=n_pos] == "S" {
        -lat_deg
    } else {
        lat_deg
    };
    let lon = if &token[w_pos..=w_pos] == "E" {
        lon_deg
    } else {
        -lon_deg
    };
    Some((lat, lon))
}

fn parse_ddmmn_dddmmw(token: &str) -> Option<(f64, f64)> {
    let n_pos = token.find('N').or_else(|| token.find('S'))?;
    let w_pos = token.find('W').or_else(|| token.find('E'))?;
    if n_pos < 4 || w_pos <= n_pos + 1 {
        return None;
    }
    let lat_raw = &token[..n_pos];
    let lon_raw = &token[n_pos + 1..w_pos];
    if lat_raw.len() != 4 || lon_raw.len() != 5 {
        return None;
    }
    let lat_deg = lat_raw[..2].parse::<f64>().ok()?;
    let lat_min = lat_raw[2..].parse::<f64>().ok()?;
    let lon_deg = lon_raw[..3].parse::<f64>().ok()?;
    let lon_min = lon_raw[3..].parse::<f64>().ok()?;
    let lat = lat_deg + lat_min / 60.0;
    let lon = lon_deg + lon_min / 60.0;
    let lat = if &token[n_pos..=n_pos] == "S" { -lat } else { lat };
    let lon = if &token[w_pos..=w_pos] == "E" { lon } else { -lon };
    Some((lat, lon))
}

fn parse_levels(levels_text: &str) -> Vec<u16> {
    levels_text
        .split_whitespace()
        .filter_map(|tok| tok.parse::<u16>().ok())
        .collect()
}

fn parse_nar_routes(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|s| s.trim_matches('-').to_string())
        .filter(|s| !s.is_empty() && s != "NIL")
        .collect()
}
