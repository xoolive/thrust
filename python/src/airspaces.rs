use pyo3::{exceptions::PyOSError, prelude::*, types::PyDict};
use serde_json::Value;
use std::path::PathBuf;
use thrust::data::eurocontrol::aixm::airspace::parse_airspace_zip_file;
use thrust::data::eurocontrol::ddr::airspaces::{
    find_file_with_prefix_suffix, parse_are_file, parse_sls_file, DdrSectorLayer,
};
use thrust::data::faa::arcgis::{
    parse_faa_airspace_boundary, parse_faa_class_airspace, parse_faa_prohibited_airspace, parse_faa_route_airspace,
    parse_faa_special_use_airspace, FaaFeature,
};
use thrust::data::faa::nasr::parse_airspaces_from_nasr_bytes;

#[pyclass(get_all)]
#[derive(Debug, Clone)]
pub struct AirspaceRecord {
    designator: String,
    name: Option<String>,
    type_: Option<String>,
    lower: Option<f64>,
    upper: Option<f64>,
    coordinates: Vec<(f64, f64)>,
    source: String,
}

#[pymethods]
impl AirspaceRecord {
    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let d = PyDict::new(py);
        d.set_item("designator", &self.designator)?;
        d.set_item("source", &self.source)?;
        d.set_item("coordinates", &self.coordinates)?;
        if let Some(name) = &self.name {
            d.set_item("name", name)?;
        }
        if let Some(type_) = &self.type_ {
            d.set_item("type", type_)?;
        }
        if let Some(lower) = self.lower {
            d.set_item("lower", lower)?;
        }
        if let Some(upper) = self.upper {
            d.set_item("upper", upper)?;
        }
        Ok(d.into())
    }
}

#[pyclass]
pub struct AixmAirspacesSource {
    airspaces: Vec<AirspaceRecord>,
}

#[pymethods]
impl AixmAirspacesSource {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let zip_path = path.join("Airspace.BASELINE.zip");
        let parsed = parse_airspace_zip_file(zip_path).map_err(|e| PyOSError::new_err(e.to_string()))?;

        let mut airspaces = Vec::new();
        for (_id, airspace) in parsed {
            let designator = airspace
                .designator
                .clone()
                .unwrap_or_else(|| airspace.identifier.clone());
            for volume in airspace.volumes {
                if volume.polygon.len() < 3 {
                    continue;
                }
                let coordinates = volume
                    .polygon
                    .into_iter()
                    .map(|(lat, lon)| (lon, lat))
                    .collect::<Vec<_>>();

                airspaces.push(AirspaceRecord {
                    designator: designator.clone(),
                    name: airspace.name.clone(),
                    type_: airspace.type_.clone(),
                    lower: volume.lower_limit.and_then(|v| v.parse::<f64>().ok()),
                    upper: volume.upper_limit.and_then(|v| v.parse::<f64>().ok()),
                    coordinates,
                    source: "eurocontrol_aixm".to_string(),
                });
            }
        }

        Ok(Self { airspaces })
    }

    fn list_airspaces(&self) -> Vec<AirspaceRecord> {
        self.airspaces.clone()
    }

    fn resolve_airspace(&self, designator: String) -> Vec<AirspaceRecord> {
        let key = designator.to_uppercase();
        self.airspaces
            .iter()
            .filter(|a| a.designator.to_uppercase() == key)
            .cloned()
            .collect()
    }
}

#[pyclass]
pub struct DdrAirspacesSource {
    airspaces: Vec<AirspaceRecord>,
}

#[pyclass]
pub struct AixmFraAirspacesSource {
    airspaces: Vec<AirspaceRecord>,
}

#[pymethods]
impl AixmFraAirspacesSource {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let base = AixmAirspacesSource::new(path)?;
        let airspaces = base
            .airspaces
            .into_iter()
            .filter(|a| {
                let d = a.designator.to_uppercase();
                let n = a.name.clone().unwrap_or_default().to_uppercase();
                let t = a.type_.clone().unwrap_or_default().to_uppercase();
                d.contains("FRA") || n.contains("FRA") || t.contains("FRA")
            })
            .collect();
        Ok(Self { airspaces })
    }

    fn list_airspaces(&self) -> Vec<AirspaceRecord> {
        self.airspaces.clone()
    }

    fn resolve_airspace(&self, designator: String) -> Vec<AirspaceRecord> {
        let key = designator.to_uppercase();
        self.airspaces
            .iter()
            .filter(|a| a.designator.to_uppercase() == key)
            .cloned()
            .collect()
    }
}

#[pyclass]
pub struct DdrFraAirspacesSource {
    airspaces: Vec<AirspaceRecord>,
}

#[pymethods]
impl DdrFraAirspacesSource {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let root = path.as_path();
        let are = find_file_with_prefix_suffix(root, "Free_Route_", ".are")
            .ok_or_else(|| PyOSError::new_err("Unable to find Free_Route_*.are"))?;
        let sls = find_file_with_prefix_suffix(root, "Free_Route_", ".sls")
            .ok_or_else(|| PyOSError::new_err("Unable to find Free_Route_*.sls"))?;

        let polygons = parse_are_file(are).map_err(|e| PyOSError::new_err(e.to_string()))?;
        let layers = parse_sls_file(sls, &polygons).map_err(|e| PyOSError::new_err(e.to_string()))?;

        let airspaces = layers
            .into_iter()
            .map(|layer: DdrSectorLayer| AirspaceRecord {
                designator: layer.designator,
                name: None,
                type_: Some("FRA".to_string()),
                lower: Some(layer.lower),
                upper: Some(layer.upper),
                coordinates: layer.coordinates,
                source: "eurocontrol_ddr".to_string(),
            })
            .collect();

        Ok(Self { airspaces })
    }

    fn list_airspaces(&self) -> Vec<AirspaceRecord> {
        self.airspaces.clone()
    }

    fn resolve_airspace(&self, designator: String) -> Vec<AirspaceRecord> {
        let key = designator.to_uppercase();
        self.airspaces
            .iter()
            .filter(|a| a.designator.to_uppercase() == key)
            .cloned()
            .collect()
    }
}

#[pymethods]
impl DdrAirspacesSource {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let root = path.as_path();

        let are = find_file_with_prefix_suffix(root, "Sectors_", ".are")
            .ok_or_else(|| PyOSError::new_err("Unable to find Sectors_*.are"))?;
        let sls = find_file_with_prefix_suffix(root, "Sectors_", ".sls")
            .ok_or_else(|| PyOSError::new_err("Unable to find Sectors_*.sls"))?;

        let polygons = parse_are_file(are).map_err(|e| PyOSError::new_err(e.to_string()))?;
        let layers = parse_sls_file(sls, &polygons).map_err(|e| PyOSError::new_err(e.to_string()))?;

        let airspaces = layers
            .into_iter()
            .map(|layer: DdrSectorLayer| AirspaceRecord {
                designator: layer.designator,
                name: None,
                type_: None,
                lower: Some(layer.lower),
                upper: Some(layer.upper),
                coordinates: layer.coordinates,
                source: "eurocontrol_ddr".to_string(),
            })
            .collect();

        Ok(Self { airspaces })
    }

    fn list_airspaces(&self) -> Vec<AirspaceRecord> {
        self.airspaces.clone()
    }

    fn resolve_airspace(&self, designator: String) -> Vec<AirspaceRecord> {
        let key = designator.to_uppercase();
        self.airspaces
            .iter()
            .filter(|a| a.designator.to_uppercase() == key)
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

fn geometry_to_polygons(geometry: &Value) -> Vec<Vec<(f64, f64)>> {
    let gtype = geometry.get("type").and_then(|v| v.as_str());
    let coords = geometry.get("coordinates");

    match (gtype, coords) {
        (Some("Polygon"), Some(c)) => c
            .as_array()
            .and_then(|rings| rings.first().cloned())
            .and_then(|ring| ring.as_array().cloned())
            .map(|ring| {
                ring.into_iter()
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

fn features_to_airspaces(features: Vec<FaaFeature>) -> Vec<AirspaceRecord> {
    let mut out = Vec::new();
    for feature in features {
        let properties = feature.properties;
        let polygons = geometry_to_polygons(&feature.geometry);
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

        for polygon in polygons {
            if polygon.len() < 3 {
                continue;
            }
            out.push(AirspaceRecord {
                designator: designator.clone(),
                name: name.clone(),
                type_: type_.clone(),
                lower,
                upper,
                coordinates: polygon,
                source: "faa_arcgis".to_string(),
            });
        }
    }
    out
}

#[pyclass]
pub struct FaaAirspacesSource {
    airspaces: Vec<AirspaceRecord>,
}

#[pyclass]
pub struct NasrAirspacesSource {
    airspaces: Vec<AirspaceRecord>,
}

#[pymethods]
impl NasrAirspacesSource {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let bytes = std::fs::read(path).map_err(|e| PyOSError::new_err(e.to_string()))?;
        let parsed = parse_airspaces_from_nasr_bytes(&bytes).map_err(|e| PyOSError::new_err(e.to_string()))?;

        let airspaces = parsed
            .into_iter()
            .filter(|a| a.coordinates.len() >= 3)
            .map(|a| AirspaceRecord {
                designator: a.designator,
                name: a.name,
                type_: a.type_,
                lower: a.lower,
                upper: a.upper,
                coordinates: a.coordinates,
                source: "faa_nasr".to_string(),
            })
            .collect();

        Ok(Self { airspaces })
    }

    fn list_airspaces(&self) -> Vec<AirspaceRecord> {
        self.airspaces.clone()
    }

    fn resolve_airspace(&self, designator: String) -> Vec<AirspaceRecord> {
        let key = designator.to_uppercase();
        self.airspaces
            .iter()
            .filter(|a| a.designator.to_uppercase() == key)
            .cloned()
            .collect()
    }
}

#[pymethods]
impl FaaAirspacesSource {
    #[new]
    #[pyo3(signature = (path=None))]
    fn new(path: Option<PathBuf>) -> PyResult<Self> {
        let features: Vec<FaaFeature> = if let Some(root) = path {
            let names = [
                "faa_airspace_boundary.json",
                "faa_class_airspace.json",
                "faa_special_use_airspace.json",
                "faa_route_airspace.json",
                "faa_prohibited_airspace.json",
            ];

            let mut all = Vec::new();
            for filename in names {
                let path = root.join(filename);
                if !path.exists() {
                    continue;
                }
                let file = std::fs::File::open(path).map_err(|e| PyOSError::new_err(e.to_string()))?;
                let payload: Value = serde_json::from_reader(file).map_err(|e| PyOSError::new_err(e.to_string()))?;
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
                all.extend(features);
            }
            all
        } else {
            let mut all = Vec::new();
            all.extend(parse_faa_airspace_boundary().map_err(|e| PyOSError::new_err(e.to_string()))?);
            all.extend(parse_faa_class_airspace().map_err(|e| PyOSError::new_err(e.to_string()))?);
            all.extend(parse_faa_special_use_airspace().map_err(|e| PyOSError::new_err(e.to_string()))?);
            all.extend(parse_faa_route_airspace().map_err(|e| PyOSError::new_err(e.to_string()))?);
            all.extend(parse_faa_prohibited_airspace().map_err(|e| PyOSError::new_err(e.to_string()))?);
            all
        };

        let airspaces = features_to_airspaces(features);
        Ok(Self { airspaces })
    }

    fn list_airspaces(&self) -> Vec<AirspaceRecord> {
        self.airspaces.clone()
    }

    fn resolve_airspace(&self, designator: String) -> Vec<AirspaceRecord> {
        let key = designator.to_uppercase();
        self.airspaces
            .iter()
            .filter(|a| a.designator.to_uppercase() == key)
            .cloned()
            .collect()
    }
}

pub fn init(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, "airspaces")?;
    m.add_class::<AirspaceRecord>()?;
    m.add_class::<AixmAirspacesSource>()?;
    m.add_class::<AixmFraAirspacesSource>()?;
    m.add_class::<DdrAirspacesSource>()?;
    m.add_class::<DdrFraAirspacesSource>()?;
    m.add_class::<FaaAirspacesSource>()?;
    m.add_class::<NasrAirspacesSource>()?;
    Ok(m)
}
