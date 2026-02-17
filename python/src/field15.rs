use pyo3::{exceptions::PyOSError, prelude::*, types::PyDict};
use thrust::data::eurocontrol::database::{AirwayDatabase, ResolvedPoint, ResolvedRouteSegment};
use thrust::data::field15::Field15Parser;

#[pyclass]
pub struct AiracDatabase {
    database: AirwayDatabase,
}

#[pyclass(get_all)]
#[derive(Debug, Clone)]
pub struct Point {
    latitude: f64,
    longitude: f64,
    name: Option<String>,
}

#[pymethods]
impl Point {
    fn __repr__(&self) -> String {
        format!(
            "Point(latitude={:.6}, longitude={:.6}{})",
            self.latitude,
            self.longitude,
            match &self.name {
                Some(name) => format!(", name='{}'", name),
                None => String::new(),
            }
        )
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let d = PyDict::new(py);
        d.set_item("latitude", self.latitude)?;
        d.set_item("longitude", self.longitude)?;
        if let Some(name) = &self.name {
            d.set_item("name", name)?;
        }
        Ok(d.into())
    }
}

impl From<ResolvedPoint> for Point {
    fn from(point: ResolvedPoint) -> Self {
        match point {
            ResolvedPoint::AirportHeliport(airport) => Self {
                latitude: airport.latitude,
                longitude: airport.longitude,
                name: Some(airport.icao),
            },
            ResolvedPoint::Navaid(navaid) => Self {
                latitude: navaid.latitude,
                longitude: navaid.longitude,
                name: navaid.name,
            },
            ResolvedPoint::DesignatedPoint(designated_point) => Self {
                latitude: designated_point.latitude,
                longitude: designated_point.longitude,
                name: Some(designated_point.designator),
            },
            ResolvedPoint::Coordinates { latitude, longitude } => Self {
                latitude,
                longitude,
                name: None,
            },
            ResolvedPoint::None => Self {
                latitude: 0.0,
                longitude: 0.0,
                name: None,
            },
        }
    }
}

#[pyclass(get_all)]
#[derive(Debug, Clone)]
pub struct Segment {
    start: Point,
    end: Point,
    name: Option<String>,
}

#[pymethods]
impl Segment {
    fn __repr__(&self) -> String {
        format!(
            "Segment(start={}, end={}{})",
            self.start.__repr__(),
            self.end.__repr__(),
            match &self.name {
                Some(name) => format!(", name='{}'", name),
                None => String::new(),
            }
        )
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let d = PyDict::new(py);
        d.set_item("start", self.start.to_dict(py)?)?;
        d.set_item("end", self.end.to_dict(py)?)?;
        if let Some(name) = &self.name {
            d.set_item("name", name)?;
        }
        Ok(d.into())
    }
}

impl From<ResolvedRouteSegment> for Segment {
    fn from(segment: ResolvedRouteSegment) -> Self {
        Self {
            start: Point::from(segment.start),
            end: Point::from(segment.end),
            name: segment.name,
        }
    }
}

#[pymethods]
impl AiracDatabase {
    #[new]
    fn new(path: String) -> PyResult<Self> {
        let database =
            AirwayDatabase::new(std::path::Path::new(&path)).map_err(|e| PyOSError::new_err(e.to_string()))?;
        Ok(Self { database })
    }

    fn enrich_route(&self, route: String) -> Vec<Segment> {
        let elements = Field15Parser::parse(&route);
        let enriched = self.database.enrich_route(elements);
        enriched.into_iter().map(Segment::from).collect()
    }
}

pub fn init(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, "field15")?;
    m.add_class::<AiracDatabase>()?;
    m.add_class::<Point>()?;
    m.add_class::<Segment>()?;
    Ok(m)
}
