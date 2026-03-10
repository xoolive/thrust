use crate::error::ThrustError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "net")]
const OPENDATA_BASE: &str = "https://opendata.arcgis.com/datasets";

const ATS_ROUTE_DATASET: &str = "acf64966af5f48a1a40fdbcb31238ba7_0";
const DESIGNATED_POINTS_DATASET: &str = "861043a88ff4486c97c3789e7dcdccc6_0";
const NAVAID_COMPONENTS_DATASET: &str = "c9254c171b6741d3a5e494860761443a_0";
const AIRSPACE_BOUNDARY_DATASET: &str = "67885972e4e940b2aa6d74024901c561_0";
const CLASS_AIRSPACE_DATASET: &str = "c6a62360338e408cb1512366ad61559e_0";
const SPECIAL_USE_AIRSPACE_DATASET: &str = "dd0d1b726e504137ab3c41b21835d05b_0";
const ROUTE_AIRSPACE_DATASET: &str = "8bf861bb9b414f4ea9f0ff2ca0f1a851_0";
const PROHIBITED_AIRSPACE_DATASET: &str = "354ee0c77484461198ebf728a2fca50c_0";

/// A GeoJSON feature from FAA's ArcGIS Open Data platform.
///
/// Features represent geographic entities published by the FAA (e.g., ATS routes, airspace boundaries).
/// Each feature contains properties (metadata) and geometry (spatial shape in GeoJSON format).
/// Feature structures vary by dataset; properties are stored as JSON values for flexibility.
///
/// # Fields
/// - `properties`: Metadata fields specific to the feature type (name, identifier, regulations, etc.)
/// - `geometry`: GeoJSON geometry object (Point, LineString, Polygon, or MultiPolygon)
///
/// # Example
/// ```ignore
/// let routes = parse_faa_ats_routes()?;
/// for route in routes {
///     if let Some(name) = route.properties.get("name") {
///         println!("Route: {}", name);
///     }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FaaFeature {
    pub properties: Value,
    pub geometry: Value,
}

/// Complete collection of FAA OpenData from ArcGIS, including routes and airspace.
///
/// This struct aggregates all major FAA navigational and airspace datasets from the ArcGIS Open Data platform,
/// which are sourced from the National Airspace System (NAS) and published by the FAA. Each field contains
/// GeoJSON features for a specific category of navigational or regulatory entity.
///
/// # Fields
/// - `ats_routes`: Automatic Terminal System (ATS) routes (airways like "J500", "L738")
/// - `designated_points`: Published waypoints and fixes (e.g., "NERTY", "ELCOB")
/// - `navaid_components`: Radio navigation aids (VOR, NDB, etc.)
/// - `airspace_boundary`: Boundaries of Class A–D airspace, TRSAs, MOAs
/// - `class_airspace`: Controlled airspace classification zones
/// - `special_use_airspace`: Military Operations Areas (MOAs), restricted areas, etc.
/// - `route_airspace`: Airspace corridors and flight corridors
/// - `prohibited_airspace`: No-fly zones around sensitive locations
///
/// # Example
/// ```ignore
/// let all_data = parse_all_faa_open_data()?;
/// println!("ATS routes: {}", all_data.ats_routes.len());
/// println!("Designated points: {}", all_data.designated_points.len());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FaaOpenData {
    pub ats_routes: Vec<FaaFeature>,
    pub designated_points: Vec<FaaFeature>,
    pub navaid_components: Vec<FaaFeature>,
    pub airspace_boundary: Vec<FaaFeature>,
    pub class_airspace: Vec<FaaFeature>,
    pub special_use_airspace: Vec<FaaFeature>,
    pub route_airspace: Vec<FaaFeature>,
    pub prohibited_airspace: Vec<FaaFeature>,
}

fn fetch_geojson(dataset_id: &str) -> Result<Vec<FaaFeature>, ThrustError> {
    #[cfg(not(feature = "net"))]
    {
        let _ = dataset_id;
        Err("FAA ArcGIS network fetch is disabled; enable feature 'net'".into())
    }

    #[cfg(feature = "net")]
    {
        let url = format!("{OPENDATA_BASE}/{dataset_id}.geojson");
        let payload = reqwest::blocking::get(url)?.error_for_status()?.json::<Value>()?;

        let features = payload
            .get("features")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|feature| FaaFeature {
                        properties: feature.get("properties").cloned().unwrap_or(Value::Null),
                        geometry: feature.get("geometry").cloned().unwrap_or(Value::Null),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(features)
    }
}

pub fn parse_faa_ats_routes() -> Result<Vec<FaaFeature>, ThrustError> {
    fetch_geojson(ATS_ROUTE_DATASET)
}

pub fn parse_faa_designated_points() -> Result<Vec<FaaFeature>, ThrustError> {
    fetch_geojson(DESIGNATED_POINTS_DATASET)
}

pub fn parse_faa_navaid_components() -> Result<Vec<FaaFeature>, ThrustError> {
    fetch_geojson(NAVAID_COMPONENTS_DATASET)
}

pub fn parse_faa_airspace_boundary() -> Result<Vec<FaaFeature>, ThrustError> {
    fetch_geojson(AIRSPACE_BOUNDARY_DATASET)
}

pub fn parse_faa_class_airspace() -> Result<Vec<FaaFeature>, ThrustError> {
    fetch_geojson(CLASS_AIRSPACE_DATASET)
}

pub fn parse_faa_special_use_airspace() -> Result<Vec<FaaFeature>, ThrustError> {
    fetch_geojson(SPECIAL_USE_AIRSPACE_DATASET)
}

pub fn parse_faa_route_airspace() -> Result<Vec<FaaFeature>, ThrustError> {
    fetch_geojson(ROUTE_AIRSPACE_DATASET)
}

pub fn parse_faa_prohibited_airspace() -> Result<Vec<FaaFeature>, ThrustError> {
    fetch_geojson(PROHIBITED_AIRSPACE_DATASET)
}

pub fn parse_all_faa_open_data() -> Result<FaaOpenData, ThrustError> {
    Ok(FaaOpenData {
        ats_routes: parse_faa_ats_routes()?,
        designated_points: parse_faa_designated_points()?,
        navaid_components: parse_faa_navaid_components()?,
        airspace_boundary: parse_faa_airspace_boundary()?,
        class_airspace: parse_faa_class_airspace()?,
        special_use_airspace: parse_faa_special_use_airspace()?,
        route_airspace: parse_faa_route_airspace()?,
        prohibited_airspace: parse_faa_prohibited_airspace()?,
    })
}
