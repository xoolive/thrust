use pyo3::{exceptions::PyOSError, prelude::*, types::PyDict};
use serde_json::Value;
use std::fs::File;
use thrust::data::eurocontrol::aixm::designated_point::parse_designated_point_zip_file;
use thrust::data::eurocontrol::aixm::navaid::parse_navaid_zip_file;
use thrust::data::eurocontrol::ddr::navpoints::parse_navpoints_dir;
use thrust::data::faa::nasr::parse_field15_data_from_nasr_zip;

#[pyclass(get_all)]
#[derive(Debug, Clone)]
pub struct NavpointRecord {
    code: String,
    kind: String,
    latitude: f64,
    longitude: f64,
    name: Option<String>,
    identifier: Option<String>,
    point_type: Option<String>,
    description: Option<String>,
    frequency: Option<f64>,
    region: Option<String>,
    source: String,
}

#[pymethods]
impl NavpointRecord {
    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let d = PyDict::new(py);
        d.set_item("code", &self.code)?;
        d.set_item("kind", &self.kind)?;
        d.set_item("latitude", self.latitude)?;
        d.set_item("longitude", self.longitude)?;
        d.set_item("source", &self.source)?;
        if let Some(name) = &self.name {
            d.set_item("name", name)?;
        }
        if let Some(identifier) = &self.identifier {
            d.set_item("identifier", identifier)?;
        }
        if let Some(point_type) = &self.point_type {
            d.set_item("point_type", point_type)?;
        }
        if let Some(description) = &self.description {
            d.set_item("description", description)?;
        }
        if let Some(frequency) = self.frequency {
            d.set_item("frequency", frequency)?;
        }
        if let Some(region) = &self.region {
            d.set_item("region", region)?;
        }
        Ok(d.into())
    }
}

#[pyclass]
pub struct AixmNavpointsSource {
    points: Vec<NavpointRecord>,
}

#[pymethods]
impl AixmNavpointsSource {
    #[new]
    fn new(path: String) -> PyResult<Self> {
        let root = std::path::Path::new(&path);
        let designated = parse_designated_point_zip_file(root.join("DesignatedPoint.BASELINE.zip"))
            .map_err(|e| PyOSError::new_err(e.to_string()))?;
        let navaids =
            parse_navaid_zip_file(root.join("Navaid.BASELINE.zip")).map_err(|e| PyOSError::new_err(e.to_string()))?;

        let mut points: Vec<NavpointRecord> = designated
            .into_values()
            .map(|point| NavpointRecord {
                code: point.designator.to_uppercase(),
                kind: "fix".to_string(),
                latitude: point.latitude,
                longitude: point.longitude,
                name: point.name,
                identifier: Some(point.identifier),
                point_type: Some(point.r#type),
                description: None,
                frequency: None,
                region: None,
                source: "eurocontrol_aixm".to_string(),
            })
            .collect();

        points.extend(navaids.into_values().filter_map(|navaid| {
            let designator = navaid.name.clone()?;
            Some(NavpointRecord {
                code: designator.to_uppercase(),
                kind: "navaid".to_string(),
                latitude: navaid.latitude,
                longitude: navaid.longitude,
                name: navaid.name,
                identifier: Some(navaid.identifier),
                point_type: Some(navaid.r#type),
                description: navaid.description,
                frequency: None,
                region: None,
                source: "eurocontrol_aixm".to_string(),
            })
        }));

        Ok(Self { points })
    }

    fn resolve_point(&self, code: String, kind: Option<String>) -> Vec<NavpointRecord> {
        let upper = code.to_uppercase();
        self.points
            .iter()
            .filter(|record| record.code == upper)
            .filter(|record| match &kind {
                Some(filter) => record.kind == filter.as_str(),
                None => true,
            })
            .cloned()
            .collect()
    }

    fn list_points(&self, kind: Option<String>) -> Vec<NavpointRecord> {
        self.points
            .iter()
            .filter(|record| match &kind {
                Some(filter) => record.kind == filter.as_str(),
                None => true,
            })
            .cloned()
            .collect()
    }
}

#[pyclass]
pub struct NasrNavpointsSource {
    points: Vec<NavpointRecord>,
}

#[pymethods]
impl NasrNavpointsSource {
    #[new]
    fn new(path: String) -> PyResult<Self> {
        let data = parse_field15_data_from_nasr_zip(path).map_err(|e| PyOSError::new_err(e.to_string()))?;

        let points = data
            .points
            .into_iter()
            .filter_map(|point| {
                let kind = match point.kind.as_str() {
                    "FIX" => Some("fix".to_string()),
                    "NAVAID" => Some("navaid".to_string()),
                    _ => None,
                }?;

                let code = point
                    .identifier
                    .split(':')
                    .next()
                    .unwrap_or(point.identifier.as_str())
                    .to_uppercase();

                Some(NavpointRecord {
                    code,
                    kind,
                    latitude: point.latitude,
                    longitude: point.longitude,
                    name: point.name,
                    identifier: Some(point.identifier),
                    point_type: point.point_type.or(Some(point.kind)),
                    description: point.description,
                    frequency: point.frequency,
                    region: point.region,
                    source: "faa_nasr".to_string(),
                })
            })
            .collect();

        Ok(Self { points })
    }

    fn resolve_point(&self, code: String, kind: Option<String>) -> Vec<NavpointRecord> {
        let upper = code.to_uppercase();
        self.points
            .iter()
            .filter(|record| record.code == upper)
            .filter(|record| match &kind {
                Some(filter) => record.kind == filter.as_str(),
                None => true,
            })
            .cloned()
            .collect()
    }

    fn list_points(&self, kind: Option<String>) -> Vec<NavpointRecord> {
        self.points
            .iter()
            .filter(|record| match &kind {
                Some(filter) => record.kind == filter.as_str(),
                None => true,
            })
            .cloned()
            .collect()
    }
}

fn value_to_f64(v: Option<&Value>) -> Option<f64> {
    v.and_then(|x| x.as_f64().or_else(|| x.as_i64().map(|n| n as f64)))
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
pub struct FaaArcgisNavpointsSource {
    points: Vec<NavpointRecord>,
}

#[pymethods]
impl FaaArcgisNavpointsSource {
    #[new]
    fn new(path: String) -> PyResult<Self> {
        let root = std::path::Path::new(&path);

        let mut points = Vec::new();

        let designated =
            read_features(&root.join("faa_designated_points.json")).map_err(|e| PyOSError::new_err(e.to_string()))?;
        for feature in designated {
            let props = feature.get("properties").unwrap_or(&Value::Null);
            let code = value_to_string(props.get("IDENT")).unwrap_or_default().to_uppercase();
            if code.is_empty() {
                continue;
            }
            let lat = value_to_f64(props.get("LATITUDE")).unwrap_or(0.0);
            let lon = value_to_f64(props.get("LONGITUDE")).unwrap_or(0.0);
            points.push(NavpointRecord {
                code: code.clone(),
                kind: "fix".to_string(),
                latitude: lat,
                longitude: lon,
                name: Some(code),
                identifier: value_to_string(props.get("IDENT")),
                point_type: value_to_string(props.get("TYPE_CODE")),
                description: value_to_string(props.get("REMARKS")),
                frequency: None,
                region: value_to_string(props.get("US_AREA")).or_else(|| value_to_string(props.get("STATE"))),
                source: "faa_arcgis".to_string(),
            });
        }

        let navaid =
            read_features(&root.join("faa_navaid_components.json")).map_err(|e| PyOSError::new_err(e.to_string()))?;
        for feature in navaid {
            let props = feature.get("properties").unwrap_or(&Value::Null);
            let code = value_to_string(props.get("IDENT")).unwrap_or_default().to_uppercase();
            if code.is_empty() {
                continue;
            }
            let lat = value_to_f64(props.get("LATITUDE")).unwrap_or(0.0);
            let lon = value_to_f64(props.get("LONGITUDE")).unwrap_or(0.0);
            points.push(NavpointRecord {
                code,
                kind: "navaid".to_string(),
                latitude: lat,
                longitude: lon,
                name: value_to_string(props.get("NAME")),
                identifier: value_to_string(props.get("IDENT")),
                point_type: value_to_string(props.get("NAV_TYPE")).or_else(|| value_to_string(props.get("TYPE_CODE"))),
                description: value_to_string(props.get("NAME")),
                frequency: value_to_f64(props.get("FREQUENCY")),
                region: value_to_string(props.get("US_AREA")),
                source: "faa_arcgis".to_string(),
            });
        }

        Ok(Self { points })
    }

    fn resolve_point(&self, code: String, kind: Option<String>) -> Vec<NavpointRecord> {
        let upper = code.to_uppercase();
        self.points
            .iter()
            .filter(|record| record.code == upper)
            .filter(|record| match &kind {
                Some(filter) => record.kind == filter.as_str(),
                None => true,
            })
            .cloned()
            .collect()
    }

    fn list_points(&self, kind: Option<String>) -> Vec<NavpointRecord> {
        self.points
            .iter()
            .filter(|record| match &kind {
                Some(filter) => record.kind == filter.as_str(),
                None => true,
            })
            .cloned()
            .collect()
    }
}

#[pyclass]
pub struct DdrNavpointsSource {
    points: Vec<NavpointRecord>,
}

#[pymethods]
impl DdrNavpointsSource {
    #[new]
    fn new(path: String) -> PyResult<Self> {
        let parsed = parse_navpoints_dir(path).map_err(|e| PyOSError::new_err(e.to_string()))?;
        let points = parsed
            .into_iter()
            .map(|point| {
                let kind = {
                    let t = point.point_type.to_uppercase();
                    if t.contains("FIX") || t == "WPT" {
                        "fix".to_string()
                    } else {
                        "navaid".to_string()
                    }
                };
                NavpointRecord {
                    code: point.name.to_uppercase(),
                    kind,
                    latitude: point.latitude,
                    longitude: point.longitude,
                    name: Some(point.name.clone()),
                    identifier: Some(point.name),
                    point_type: Some(point.point_type),
                    description: point.description,
                    frequency: None,
                    region: None,
                    source: "eurocontrol_ddr".to_string(),
                }
            })
            .collect();

        Ok(Self { points })
    }

    fn resolve_point(&self, code: String, kind: Option<String>) -> Vec<NavpointRecord> {
        let upper = code.to_uppercase();
        self.points
            .iter()
            .filter(|record| record.code == upper)
            .filter(|record| match &kind {
                Some(filter) => record.kind == filter.as_str(),
                None => true,
            })
            .cloned()
            .collect()
    }

    fn list_points(&self, kind: Option<String>) -> Vec<NavpointRecord> {
        self.points
            .iter()
            .filter(|record| match &kind {
                Some(filter) => record.kind == filter.as_str(),
                None => true,
            })
            .cloned()
            .collect()
    }
}

pub fn init(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, "navpoints")?;
    m.add_class::<NavpointRecord>()?;
    m.add_class::<AixmNavpointsSource>()?;
    m.add_class::<NasrNavpointsSource>()?;
    m.add_class::<FaaArcgisNavpointsSource>()?;
    m.add_class::<DdrNavpointsSource>()?;
    Ok(m)
}
