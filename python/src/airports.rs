use pyo3::{exceptions::PyOSError, prelude::*, types::PyDict};
use serde_json::Value;
use std::fs::File;
use std::path::PathBuf;
use thrust::data::eurocontrol::aixm::dataset::parse_aixm_folder_path;
use thrust::data::eurocontrol::ddr::airports::parse_airports_path;
use thrust::data::faa::nasr::parse_field15_data_from_nasr_zip;

#[pyclass(get_all)]
#[derive(Debug, Clone)]
pub struct AirportRecord {
    code: String,
    latitude: f64,
    longitude: f64,
    altitude: Option<f64>,
    iata: Option<String>,
    icao: Option<String>,
    name: Option<String>,
    country: Option<String>,
    source: String,
}

#[pymethods]
impl AirportRecord {
    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let d = PyDict::new(py);
        d.set_item("code", &self.code)?;
        d.set_item("latitude", self.latitude)?;
        d.set_item("longitude", self.longitude)?;
        d.set_item("source", &self.source)?;
        if let Some(altitude) = self.altitude {
            d.set_item("altitude", altitude)?;
        }
        if let Some(iata) = &self.iata {
            d.set_item("iata", iata)?;
        }
        if let Some(icao) = &self.icao {
            d.set_item("icao", icao)?;
        }
        if let Some(name) = &self.name {
            d.set_item("name", name)?;
        }
        if let Some(country) = &self.country {
            d.set_item("country", country)?;
        }
        Ok(d.into())
    }
}

#[pyclass]
pub struct AixmAirportsSource {
    airports: Vec<AirportRecord>,
}

#[pymethods]
impl AixmAirportsSource {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let airports = parse_aixm_folder_path(path)
            .map_err(|e| PyOSError::new_err(e.to_string()))?
            .airports
            .into_iter()
            .map(|airport| AirportRecord {
                code: airport.code,
                latitude: airport.latitude,
                longitude: airport.longitude,
                altitude: None,
                iata: airport.iata,
                icao: airport.icao,
                name: airport.name,
                country: None,
                source: airport.source,
            })
            .collect();

        Ok(Self { airports })
    }

    fn resolve_airport(&self, code: String) -> Vec<AirportRecord> {
        let upper = code.to_uppercase();
        self.airports
            .iter()
            .filter(|record| record.code == upper)
            .cloned()
            .collect()
    }

    fn list_airports(&self) -> Vec<AirportRecord> {
        self.airports.clone()
    }
}

#[pyclass]
pub struct NasrAirportsSource {
    airports: Vec<AirportRecord>,
}

#[pymethods]
impl NasrAirportsSource {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let data = parse_field15_data_from_nasr_zip(path).map_err(|e| PyOSError::new_err(e.to_string()))?;

        let airports = data
            .points
            .into_iter()
            .filter(|point| point.kind == "AIRPORT")
            .map(|point| AirportRecord {
                code: point.identifier.clone().to_uppercase(),
                latitude: point.latitude,
                longitude: point.longitude,
                altitude: None,
                iata: Some(point.identifier.clone().to_uppercase()),
                icao: Some(point.identifier.to_uppercase()),
                name: point.name,
                country: None,
                source: "faa_nasr".to_string(),
            })
            .collect();

        Ok(Self { airports })
    }

    fn resolve_airport(&self, code: String) -> Vec<AirportRecord> {
        let upper = code.to_uppercase();
        self.airports
            .iter()
            .filter(|record| record.code == upper)
            .cloned()
            .collect()
    }

    fn list_airports(&self) -> Vec<AirportRecord> {
        self.airports.clone()
    }
}

fn value_to_f64(v: Option<&Value>) -> Option<f64> {
    v.and_then(|x| x.as_f64().or_else(|| x.as_i64().map(|n| n as f64)))
}

fn value_to_string(v: Option<&Value>) -> Option<String> {
    v.and_then(|x| x.as_str().map(|s| s.to_string()))
}

fn parse_coord(value: &Value) -> Option<f64> {
    if let Some(v) = value.as_f64() {
        return Some(v);
    }
    let s = value.as_str()?.trim();
    // FAA ArcGIS airport strings look like 51-53-00.8980N
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

#[pyclass]
pub struct DdrAirportsSource {
    airports: Vec<AirportRecord>,
}

#[pymethods]
impl DdrAirportsSource {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let parsed = parse_airports_path(path).map_err(|e| PyOSError::new_err(e.to_string()))?;
        let airports = parsed
            .into_iter()
            .map(|airport| AirportRecord {
                code: airport.code.clone(),
                latitude: airport.latitude,
                longitude: airport.longitude,
                altitude: None,
                iata: None,
                icao: Some(airport.code),
                name: None,
                country: None,
                source: "eurocontrol_ddr".to_string(),
            })
            .collect();

        Ok(Self { airports })
    }

    fn resolve_airport(&self, code: String) -> Vec<AirportRecord> {
        let upper = code.to_uppercase();
        self.airports
            .iter()
            .filter(|record| record.code == upper)
            .cloned()
            .collect()
    }

    fn list_airports(&self) -> Vec<AirportRecord> {
        self.airports.clone()
    }
}

#[pyclass]
pub struct FaaArcgisAirportsSource {
    airports: Vec<AirportRecord>,
}

#[pymethods]
impl FaaArcgisAirportsSource {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let file = File::open(path.join("faa_airports.json")).map_err(|e| PyOSError::new_err(e.to_string()))?;
        let payload: Value = serde_json::from_reader(file).map_err(|e| PyOSError::new_err(e.to_string()))?;

        let mut airports = Vec::new();
        for feat in payload
            .get("features")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default()
        {
            let props = feat.get("properties").unwrap_or(&Value::Null);
            let ident = value_to_string(props.get("IDENT")).unwrap_or_default().to_uppercase();
            let icao = value_to_string(props.get("ICAO_ID")).map(|x| x.to_uppercase());
            if ident.is_empty() && icao.is_none() {
                continue;
            }

            let latitude = props.get("LATITUDE").and_then(parse_coord);
            let longitude = props.get("LONGITUDE").and_then(parse_coord);
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

            airports.push(AirportRecord {
                code,
                latitude,
                longitude,
                altitude: value_to_f64(props.get("ELEVATION")),
                iata: if ident.len() == 3 { Some(ident) } else { None },
                icao,
                name: value_to_string(props.get("NAME")),
                country: None,
                source: "faa_arcgis".to_string(),
            });
        }

        Ok(Self { airports })
    }

    fn resolve_airport(&self, code: String) -> Vec<AirportRecord> {
        let upper = code.to_uppercase();
        self.airports
            .iter()
            .filter(|record| record.code == upper)
            .cloned()
            .collect()
    }

    fn list_airports(&self) -> Vec<AirportRecord> {
        self.airports.clone()
    }
}

pub fn init(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, "airports")?;
    m.add_class::<AirportRecord>()?;
    m.add_class::<AixmAirportsSource>()?;
    m.add_class::<NasrAirportsSource>()?;
    m.add_class::<DdrAirportsSource>()?;
    m.add_class::<FaaArcgisAirportsSource>()?;
    Ok(m)
}
