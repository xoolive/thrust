use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct AirportRecord {
    pub code: String,
    pub iata: Option<String>,
    pub icao: Option<String>,
    pub name: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub region: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NavpointRecord {
    pub code: String,
    pub identifier: String,
    pub kind: String,
    pub name: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub description: Option<String>,
    pub frequency: Option<f64>,
    pub point_type: Option<String>,
    pub region: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AirwayPointRecord {
    pub code: String,
    pub raw_code: String,
    pub kind: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AirwayRecord {
    pub name: String,
    pub source: String,
    pub points: Vec<AirwayPointRecord>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AirspaceRecord {
    pub designator: String,
    pub name: Option<String>,
    pub type_: Option<String>,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub coordinates: Vec<(f64, f64)>,
    pub source: String,
}

pub(crate) fn normalize_airway_name(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}

pub(crate) fn normalize_point_code(value: &str) -> String {
    value.split(':').next().unwrap_or(value).to_uppercase()
}

pub(crate) fn point_kind(kind: &str) -> String {
    match kind {
        "FIX" => "fix".to_string(),
        "NAVAID" => "navaid".to_string(),
        "AIRPORT" => "airport".to_string(),
        _ => "point".to_string(),
    }
}
