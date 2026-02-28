use pyo3::{exceptions::PyOSError, prelude::*, types::PyDict};
use serde_json::Value;
use std::fs::File;
use std::path::PathBuf;
use thrust::data::eurocontrol::database::{AirwayDatabase, ResolvedPoint, ResolvedRoute};
use thrust::data::eurocontrol::ddr::routes::parse_routes_dir;
use thrust::data::faa::nasr::parse_field15_data_from_nasr_zip;

fn normalize_name(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}

fn normalize_point_code(value: &str) -> String {
    value.split(':').next().unwrap_or(value).to_uppercase()
}

#[pyclass(get_all)]
#[derive(Debug, Clone)]
pub struct AirwayPointRecord {
    code: String,
    raw_code: Option<String>,
    kind: String,
    latitude: f64,
    longitude: f64,
}

#[pymethods]
impl AirwayPointRecord {
    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let d = PyDict::new(py);
        d.set_item("code", &self.code)?;
        if let Some(raw_code) = &self.raw_code {
            d.set_item("raw_code", raw_code)?;
        }
        d.set_item("kind", &self.kind)?;
        d.set_item("latitude", self.latitude)?;
        d.set_item("longitude", self.longitude)?;
        Ok(d.into())
    }
}

#[pyclass(get_all)]
#[derive(Debug, Clone)]
pub struct AirwayRecord {
    name: String,
    points: Vec<AirwayPointRecord>,
    source: String,
}

#[pymethods]
impl AirwayRecord {
    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let d = PyDict::new(py);
        d.set_item("name", &self.name)?;
        d.set_item("source", &self.source)?;
        let pts = self
            .points
            .iter()
            .map(|p| p.to_dict(py))
            .collect::<PyResult<Vec<_>>>()?;
        d.set_item("points", pts)?;
        Ok(d.into())
    }
}

fn point_to_record(point: &ResolvedPoint) -> Option<AirwayPointRecord> {
    match point {
        ResolvedPoint::AirportHeliport(airport) => Some(AirwayPointRecord {
            code: normalize_point_code(&airport.icao),
            raw_code: Some(airport.icao.clone()),
            kind: "airport".to_string(),
            latitude: airport.latitude,
            longitude: airport.longitude,
        }),
        ResolvedPoint::Navaid(navaid) => navaid.name.clone().map(|name| AirwayPointRecord {
            code: normalize_point_code(&name),
            raw_code: Some(name),
            kind: "navaid".to_string(),
            latitude: navaid.latitude,
            longitude: navaid.longitude,
        }),
        ResolvedPoint::DesignatedPoint(point) => Some(AirwayPointRecord {
            code: normalize_point_code(&point.designator),
            raw_code: Some(point.designator.clone()),
            kind: "fix".to_string(),
            latitude: point.latitude,
            longitude: point.longitude,
        }),
        ResolvedPoint::Coordinates { latitude, longitude } => Some(AirwayPointRecord {
            code: format!("{latitude:.6},{longitude:.6}"),
            raw_code: None,
            kind: "coordinates".to_string(),
            latitude: *latitude,
            longitude: *longitude,
        }),
        ResolvedPoint::None => None,
    }
}

fn route_to_record(route: ResolvedRoute, source: &str) -> AirwayRecord {
    let mut points: Vec<AirwayPointRecord> = Vec::new();
    for segment in route.segments {
        if let Some(start) = point_to_record(&segment.start) {
            if points.last().map(|x| &x.code) != Some(&start.code) {
                points.push(start);
            }
        }
        if let Some(end) = point_to_record(&segment.end) {
            if points.last().map(|x| &x.code) != Some(&end.code) {
                points.push(end);
            }
        }
    }
    AirwayRecord {
        name: route.name,
        points,
        source: source.to_string(),
    }
}

#[pyclass]
pub struct AixmAirwaysSource {
    database: AirwayDatabase,
}

#[pymethods]
impl AixmAirwaysSource {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let database = AirwayDatabase::new(&path).map_err(|e| PyOSError::new_err(e.to_string()))?;
        Ok(Self { database })
    }

    fn resolve_airway(&self, name: String) -> Vec<AirwayRecord> {
        ResolvedRoute::lookup(&name, &self.database)
            .into_iter()
            .map(|route| route_to_record(route, "eurocontrol_aixm"))
            .collect()
    }

    fn list_airways(&self) -> Vec<String> {
        // We cannot list all route names cheaply from private internals yet.
        Vec::new()
    }
}

#[pyclass]
pub struct NasrAirwaysSource {
    routes: Vec<AirwayRecord>,
}

#[pymethods]
impl NasrAirwaysSource {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let data = parse_field15_data_from_nasr_zip(path).map_err(|e| PyOSError::new_err(e.to_string()))?;

        let mut point_index: std::collections::HashMap<String, AirwayPointRecord> = std::collections::HashMap::new();
        for point in &data.points {
            let kind = match point.kind.as_str() {
                "FIX" => "fix",
                "NAVAID" => "navaid",
                "AIRPORT" => "airport",
                _ => "point",
            }
            .to_string();

            let record = AirwayPointRecord {
                code: normalize_point_code(&point.identifier),
                raw_code: Some(point.identifier.to_uppercase()),
                kind,
                latitude: point.latitude,
                longitude: point.longitude,
            };

            point_index.insert(point.identifier.to_uppercase(), record.clone());
            if let Some((base, _suffix)) = point.identifier.split_once(':') {
                point_index.entry(base.to_uppercase()).or_insert(record);
            }
        }

        let mut by_name: std::collections::HashMap<String, Vec<AirwayPointRecord>> = std::collections::HashMap::new();

        for seg in data.airways {
            let route_name = if seg.airway_id.trim().is_empty() {
                seg.airway_name.clone()
            } else {
                seg.airway_id.clone()
            };
            let entry = by_name.entry(route_name).or_default();
            let from_key = seg.from_point.to_uppercase();
            let to_key = seg.to_point.to_uppercase();
            let from = point_index.get(&from_key).cloned().unwrap_or(AirwayPointRecord {
                code: normalize_point_code(&from_key),
                raw_code: Some(from_key.clone()),
                kind: "point".to_string(),
                latitude: 0.0,
                longitude: 0.0,
            });
            let to = point_index.get(&to_key).cloned().unwrap_or(AirwayPointRecord {
                code: normalize_point_code(&to_key),
                raw_code: Some(to_key.clone()),
                kind: "point".to_string(),
                latitude: 0.0,
                longitude: 0.0,
            });

            if entry.last().map(|x| &x.code) != Some(&from.code) {
                entry.push(from);
            }
            if entry.last().map(|x| &x.code) != Some(&to.code) {
                entry.push(to);
            }
        }

        let routes = by_name
            .into_iter()
            .map(|(name, points)| AirwayRecord {
                name,
                points,
                source: "faa_nasr".to_string(),
            })
            .collect();
        Ok(Self { routes })
    }

    fn resolve_airway(&self, name: String) -> Vec<AirwayRecord> {
        let upper = normalize_name(&name);
        self.routes
            .iter()
            .filter(|route| normalize_name(&route.name) == upper)
            .cloned()
            .collect()
    }

    fn list_airways(&self) -> Vec<AirwayRecord> {
        self.routes.clone()
    }
}

fn value_to_string(v: Option<&Value>) -> Option<String> {
    v.and_then(|x| x.as_str().map(|s| s.to_string()))
}

fn read_features(path: &std::path::Path) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let payload: Value = serde_json::from_reader(file)?;
    Ok(payload
        .get("features")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default())
}

#[pyclass]
pub struct FaaArcgisAirwaysSource {
    routes: Vec<AirwayRecord>,
}

#[pymethods]
impl FaaArcgisAirwaysSource {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let features =
            read_features(&path.join("faa_ats_routes.json")).map_err(|e| PyOSError::new_err(e.to_string()))?;

        let mut by_name: std::collections::HashMap<String, Vec<AirwayPointRecord>> = std::collections::HashMap::new();

        for feature in features {
            let props = feature.get("properties").unwrap_or(&Value::Null);
            let route_name = value_to_string(props.get("IDENT")).unwrap_or_default().to_uppercase();
            if route_name.is_empty() {
                continue;
            }

            let geom = feature.get("geometry").unwrap_or(&Value::Null);
            if geom.get("type").and_then(|x| x.as_str()) != Some("LineString") {
                continue;
            }
            let coordinates = geom
                .get("coordinates")
                .and_then(|x| x.as_array())
                .cloned()
                .unwrap_or_default();

            let entry = by_name.entry(route_name).or_default();
            for (idx, point) in coordinates.iter().enumerate() {
                let arr = match point.as_array() {
                    Some(v) if v.len() >= 2 => v,
                    _ => continue,
                };
                let lon = arr[0].as_f64().unwrap_or(0.0);
                let lat = arr[1].as_f64().unwrap_or(0.0);
                let p = AirwayPointRecord {
                    code: format!("{}:{}", entry.len() + idx, "PT"),
                    raw_code: None,
                    kind: "point".to_string(),
                    latitude: lat,
                    longitude: lon,
                };
                if entry
                    .last()
                    .map(|x| (x.latitude, x.longitude) != (p.latitude, p.longitude))
                    .unwrap_or(true)
                {
                    entry.push(p);
                }
            }
        }

        let routes = by_name
            .into_iter()
            .map(|(name, points)| AirwayRecord {
                name,
                points,
                source: "faa_arcgis".to_string(),
            })
            .collect();

        Ok(Self { routes })
    }

    fn resolve_airway(&self, name: String) -> Vec<AirwayRecord> {
        let upper = normalize_name(&name);
        self.routes
            .iter()
            .filter(|route| normalize_name(&route.name) == upper)
            .cloned()
            .collect()
    }

    fn list_airways(&self) -> Vec<AirwayRecord> {
        self.routes.clone()
    }
}

#[pyclass]
pub struct DdrAirwaysSource {
    routes: Vec<AirwayRecord>,
}

#[pymethods]
impl DdrAirwaysSource {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let parsed = parse_routes_dir(path).map_err(|e| PyOSError::new_err(e.to_string()))?;
        let mut by_name: std::collections::HashMap<String, Vec<AirwayPointRecord>> = std::collections::HashMap::new();

        for point in parsed {
            let route = point.route.to_uppercase();
            let entry = by_name.entry(route).or_default();
            entry.push(AirwayPointRecord {
                code: point.navaid.to_uppercase(),
                raw_code: Some(point.navaid.to_uppercase()),
                kind: if point.point_type.to_uppercase().contains("FIX") {
                    "fix".to_string()
                } else {
                    "navaid".to_string()
                },
                latitude: point.latitude.unwrap_or(0.0),
                longitude: point.longitude.unwrap_or(0.0),
            });
        }

        let mut routes: Vec<AirwayRecord> = by_name
            .into_iter()
            .map(|(name, points)| AirwayRecord {
                name,
                points,
                source: "eurocontrol_ddr".to_string(),
            })
            .collect();
        routes.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(Self { routes })
    }

    fn resolve_airway(&self, name: String) -> Vec<AirwayRecord> {
        let upper = normalize_name(&name);
        self.routes
            .iter()
            .filter(|route| normalize_name(&route.name) == upper)
            .cloned()
            .collect()
    }

    fn list_airways(&self) -> Vec<AirwayRecord> {
        self.routes.clone()
    }
}

pub fn init(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, "airways")?;
    m.add_class::<AirwayPointRecord>()?;
    m.add_class::<AirwayRecord>()?;
    m.add_class::<AixmAirwaysSource>()?;
    m.add_class::<NasrAirwaysSource>()?;
    m.add_class::<FaaArcgisAirwaysSource>()?;
    m.add_class::<DdrAirwaysSource>()?;
    Ok(m)
}
