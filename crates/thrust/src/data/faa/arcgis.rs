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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcgisAirportRecord {
    pub code: String,
    pub iata: Option<String>,
    pub icao: Option<String>,
    pub name: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub region: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcgisNavpointRecord {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcgisAirwayPointRecord {
    pub code: String,
    pub raw_code: String,
    pub kind: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcgisAirwayRecord {
    pub name: String,
    pub source: String,
    pub route_class: Option<String>,
    pub points: Vec<ArcgisAirwayPointRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcgisAirspaceRecord {
    pub designator: String,
    pub name: Option<String>,
    pub type_: Option<String>,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub coordinates: Vec<(f64, f64)>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArcgisDataset {
    pub airports: Vec<ArcgisAirportRecord>,
    pub navaids: Vec<ArcgisNavpointRecord>,
    pub airways: Vec<ArcgisAirwayRecord>,
    pub airspaces: Vec<ArcgisAirspaceRecord>,
}

pub fn parse_arcgis_features(features: &[Value]) -> ArcgisDataset {
    let airports = arcgis_features_to_airports(features);
    let airspaces = arcgis_features_to_airspaces(features);
    let (fixes, mut navaids) = arcgis_features_to_navpoints(features);
    navaids.extend(fixes.iter().cloned());
    navaids.sort_by(|a, b| a.code.cmp(&b.code).then(a.point_type.cmp(&b.point_type)));
    navaids.dedup_by(|a, b| {
        a.code == b.code && a.point_type == b.point_type && a.latitude == b.latitude && a.longitude == b.longitude
    });
    let airways = arcgis_features_to_airways(features);

    ArcgisDataset {
        airports,
        navaids,
        airways,
        airspaces,
    }
}

fn value_to_f64(v: Option<&Value>) -> Option<f64> {
    v.and_then(|x| x.as_f64().or_else(|| x.as_i64().map(|n| n as f64)))
}

fn parse_coord(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    if let Some(v) = value.as_f64() {
        return Some(v);
    }
    let s = value.as_str()?.trim();
    let hemi = s.chars().last()?;
    let sign = match hemi {
        'N' | 'E' => 1.0,
        'S' | 'W' => -1.0,
        _ => 1.0,
    };
    let core = s.strip_suffix(hemi).unwrap_or(s);
    let parts: Vec<&str> = core.split('-').collect();
    if parts.len() != 3 {
        return core.parse::<f64>().ok();
    }
    let deg = parts[0].parse::<f64>().ok()?;
    let min = parts[1].parse::<f64>().ok()?;
    let sec = parts[2].parse::<f64>().ok()?;
    Some(sign * (deg + min / 60.0 + sec / 3600.0))
}

fn value_to_string(v: Option<&Value>) -> Option<String> {
    v.and_then(|x| x.as_str().map(|s| s.to_string()))
}

fn value_to_i64(v: Option<&Value>) -> Option<i64> {
    v.and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|n| n as i64)))
}

fn geometry_to_polygons(geometry: &Value) -> Vec<Vec<(f64, f64)>> {
    let gtype = geometry.get("type").and_then(|v| v.as_str());
    let coords = geometry.get("coordinates");

    match (gtype, coords) {
        (Some("Polygon"), Some(c)) => c
            .as_array()
            .and_then(|rings| rings.first())
            .and_then(|ring| ring.as_array())
            .map(|ring| {
                ring.iter()
                    .filter_map(|pt| {
                        let arr = pt.as_array()?;
                        if arr.len() < 2 {
                            return None;
                        }
                        let lon = arr[0].as_f64()?;
                        let lat = arr[1].as_f64()?;
                        Some((lon, lat))
                    })
                    .collect::<Vec<_>>()
            })
            .into_iter()
            .collect(),
        (Some("MultiPolygon"), Some(c)) => c
            .as_array()
            .map(|polys| {
                polys
                    .iter()
                    .filter_map(|poly| {
                        let ring = poly.as_array()?.first()?.as_array()?;
                        Some(
                            ring.iter()
                                .filter_map(|pt| {
                                    let arr = pt.as_array()?;
                                    if arr.len() < 2 {
                                        return None;
                                    }
                                    let lon = arr[0].as_f64()?;
                                    let lat = arr[1].as_f64()?;
                                    Some((lon, lat))
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        _ => vec![],
    }
}

fn arcgis_features_to_airspaces(features: &[Value]) -> Vec<ArcgisAirspaceRecord> {
    let mut out = Vec::new();
    for feature in features {
        let properties = feature.get("properties").unwrap_or(&Value::Null);
        let geometry = feature.get("geometry").unwrap_or(&Value::Null);
        let polygons = geometry_to_polygons(geometry);
        if polygons.is_empty() {
            continue;
        }

        let designator = value_to_string(properties.get("IDENT"))
            .or_else(|| value_to_string(properties.get("NAME")))
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let name = value_to_string(properties.get("NAME"));
        let type_ = value_to_string(properties.get("TYPE_CODE"));
        let lower =
            value_to_f64(properties.get("LOWER_VAL")).map(|v| if (v + 9998.0).abs() < f64::EPSILON { 0.0 } else { v });
        let upper = value_to_f64(properties.get("UPPER_VAL")).map(|v| {
            if (v + 9998.0).abs() < f64::EPSILON {
                f64::INFINITY
            } else {
                v
            }
        });

        for coords in polygons {
            if coords.len() < 3 {
                continue;
            }
            out.push(ArcgisAirspaceRecord {
                designator: designator.clone(),
                name: name.clone(),
                type_: type_.clone(),
                lower,
                upper,
                coordinates: coords,
                source: "faa_arcgis".to_string(),
            });
        }
    }
    out
}

fn arcgis_features_to_navpoints(features: &[Value]) -> (Vec<ArcgisNavpointRecord>, Vec<ArcgisNavpointRecord>) {
    let mut fixes = Vec::new();
    let mut navaid_groups: std::collections::HashMap<String, ArcgisNavpointRecord> = std::collections::HashMap::new();
    let mut navaid_components: std::collections::HashMap<String, (bool, bool, bool, bool)> =
        std::collections::HashMap::new();

    for feature in features {
        let props = feature.get("properties").unwrap_or(&Value::Null);
        let ident = value_to_string(props.get("IDENT")).unwrap_or_default().to_uppercase();
        if ident.is_empty() {
            continue;
        }

        if props.get("NAV_TYPE").is_some() || props.get("FREQUENCY").is_some() {
            let latitude = parse_coord(props.get("LATITUDE")).unwrap_or(0.0);
            let longitude = parse_coord(props.get("LONGITUDE")).unwrap_or(0.0);
            let group_key = value_to_string(props.get("NAVSYS_ID"))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| ident.clone());

            navaid_groups
                .entry(group_key.clone())
                .or_insert_with(|| ArcgisNavpointRecord {
                    code: ident.clone(),
                    identifier: ident,
                    kind: "navaid".to_string(),
                    name: value_to_string(props.get("NAME")),
                    latitude,
                    longitude,
                    description: value_to_string(props.get("NAME")),
                    frequency: value_to_f64(props.get("FREQUENCY")),
                    point_type: value_to_string(props.get("TYPE_CODE")),
                    region: value_to_string(props.get("US_AREA")),
                    source: "faa_arcgis".to_string(),
                });

            let entry = navaid_components
                .entry(group_key)
                .or_insert((false, false, false, false));
            match value_to_i64(props.get("NAV_TYPE")) {
                Some(1) => entry.0 = true,
                Some(2) => entry.1 = true,
                Some(3) => entry.2 = true,
                Some(4) => entry.3 = true,
                _ => {}
            }
        } else {
            let latitude = parse_coord(props.get("LATITUDE")).unwrap_or(0.0);
            let longitude = parse_coord(props.get("LONGITUDE")).unwrap_or(0.0);
            fixes.push(ArcgisNavpointRecord {
                code: ident.clone(),
                identifier: ident.clone(),
                kind: "fix".to_string(),
                name: Some(ident),
                latitude,
                longitude,
                description: value_to_string(props.get("REMARKS")),
                frequency: None,
                point_type: value_to_string(props.get("TYPE_CODE")).map(|s| s.to_uppercase()),
                region: value_to_string(props.get("US_AREA")).or_else(|| value_to_string(props.get("STATE"))),
                source: "faa_arcgis".to_string(),
            });
        }
    }

    let mut navaids: Vec<ArcgisNavpointRecord> = navaid_groups
        .into_iter()
        .map(|(group_key, mut record)| {
            if let Some((has_ndb, has_dme, has_vor, has_tacan)) = navaid_components.get(&group_key).copied() {
                record.point_type = Some(
                    if has_vor && has_tacan {
                        "VORTAC"
                    } else if has_vor && has_dme {
                        "VOR_DME"
                    } else if has_vor {
                        "VOR"
                    } else if has_tacan {
                        "TACAN"
                    } else if has_dme {
                        "DME"
                    } else if has_ndb {
                        "NDB"
                    } else {
                        record.point_type.as_deref().unwrap_or("NAVAID")
                    }
                    .to_string(),
                );
            }
            record
        })
        .collect();
    navaids.sort_by(|a, b| a.code.cmp(&b.code));

    (fixes, navaids)
}

fn arcgis_features_to_airports(features: &[Value]) -> Vec<ArcgisAirportRecord> {
    let mut airports = Vec::new();

    for feature in features {
        let props = feature.get("properties").unwrap_or(&Value::Null);
        let ident = value_to_string(props.get("IDENT")).unwrap_or_default().to_uppercase();
        let icao = value_to_string(props.get("ICAO_ID")).map(|x| x.to_uppercase());
        if ident.is_empty() && icao.is_none() {
            continue;
        }

        let latitude = parse_coord(props.get("LATITUDE"));
        let longitude = parse_coord(props.get("LONGITUDE"));
        let (latitude, longitude) = match (latitude, longitude) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => continue,
        };

        let code = if ident.is_empty() {
            icao.clone().unwrap_or_default()
        } else {
            ident.clone()
        };
        if code.is_empty() {
            continue;
        }

        airports.push(ArcgisAirportRecord {
            code,
            iata: if ident.len() == 3 { Some(ident) } else { None },
            icao,
            name: value_to_string(props.get("NAME")),
            latitude,
            longitude,
            region: value_to_string(props.get("STATE")).or_else(|| value_to_string(props.get("US_AREA"))),
            source: "faa_arcgis".to_string(),
        });
    }

    airports
}

fn arcgis_features_to_airways(features: &[Value]) -> Vec<ArcgisAirwayRecord> {
    let mut grouped: std::collections::HashMap<String, Vec<ArcgisAirwayPointRecord>> = std::collections::HashMap::new();
    let mut point_id_to_ident: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for feature in features {
        let props = feature.get("properties").unwrap_or(&Value::Null);
        let global_id = value_to_string(props.get("GLOBAL_ID")).map(|s| s.to_uppercase());
        let ident = value_to_string(props.get("IDENT")).map(|s| s.to_uppercase());
        if let (Some(gid), Some(idt)) = (global_id, ident) {
            if !gid.is_empty() && !idt.is_empty() {
                point_id_to_ident.entry(gid).or_insert(idt);
            }
        }
    }

    for feature in features {
        let props = feature.get("properties").unwrap_or(&Value::Null);
        let name = value_to_string(props.get("IDENT")).unwrap_or_default().to_uppercase();
        if name.is_empty() {
            continue;
        }

        let geom = feature.get("geometry").unwrap_or(&Value::Null);
        if geom.get("type").and_then(|x| x.as_str()) != Some("LineString") {
            continue;
        }
        let coords = geom
            .get("coordinates")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();

        let start_id = value_to_string(props.get("STARTPT_ID")).map(|s| s.to_uppercase());
        let end_id = value_to_string(props.get("ENDPT_ID")).map(|s| s.to_uppercase());
        let start_code = start_id
            .as_ref()
            .and_then(|id| point_id_to_ident.get(id).cloned())
            .or(start_id.clone());
        let end_code = end_id
            .as_ref()
            .and_then(|id| point_id_to_ident.get(id).cloned())
            .or(end_id.clone());

        let entry = grouped.entry(name).or_default();
        let coord_len = coords.len();
        for (idx, p) in coords.into_iter().enumerate() {
            let arr = match p.as_array() {
                Some(v) if v.len() >= 2 => v,
                _ => continue,
            };
            let lon = arr[0].as_f64().unwrap_or(0.0);
            let lat = arr[1].as_f64().unwrap_or(0.0);
            if entry
                .last()
                .map(|x| (x.latitude, x.longitude) == (lat, lon))
                .unwrap_or(false)
            {
                continue;
            }

            let raw_code = if idx == 0 {
                start_code.clone().unwrap_or_default()
            } else if idx + 1 == coord_len {
                end_code.clone().unwrap_or_default()
            } else {
                String::new()
            };
            let code = if raw_code.is_empty() {
                format!("{},{}", lat, lon)
            } else {
                raw_code.clone()
            };

            entry.push(ArcgisAirwayPointRecord {
                code,
                raw_code,
                kind: "point".to_string(),
                latitude: lat,
                longitude: lon,
            });
        }
    }

    grouped
        .into_iter()
        .map(|(name, points)| ArcgisAirwayRecord {
            name,
            source: "faa_arcgis".to_string(),
            route_class: None,
            points,
        })
        .collect()
}
