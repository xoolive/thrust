use serde::{Deserialize, Serialize};
use thrust::data::eurocontrol::aixm::dataset as core;
use thrust::data::faa::arcgis as core_arcgis;
use thrust::data::faa::nasr as core_nasr;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub route_class: Option<String>,
    pub points: Vec<AirwayPointRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcedureRecord {
    pub name: String,
    pub source: String,
    pub procedure_kind: String,
    pub route_class: Option<String>,
    pub airport: Option<String>,
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

#[derive(Clone, Debug, Serialize)]
pub struct AirspaceLayerRecord {
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub coordinates: Vec<(f64, f64)>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AirspaceCompositeRecord {
    pub designator: String,
    pub name: Option<String>,
    pub type_: Option<String>,
    pub layers: Vec<AirspaceLayerRecord>,
    pub source: String,
}

impl From<core::AirportRecord> for AirportRecord {
    fn from(value: core::AirportRecord) -> Self {
        Self {
            code: value.code,
            iata: value.iata,
            icao: value.icao,
            name: value.name,
            latitude: value.latitude,
            longitude: value.longitude,
            region: value.region,
            source: value.source,
        }
    }
}

impl From<core::NavpointRecord> for NavpointRecord {
    fn from(value: core::NavpointRecord) -> Self {
        Self {
            code: value.code,
            identifier: value.identifier,
            kind: value.kind,
            name: value.name,
            latitude: value.latitude,
            longitude: value.longitude,
            description: value.description,
            frequency: value.frequency,
            point_type: value.point_type,
            region: value.region,
            source: value.source,
        }
    }
}

impl From<core::AirwayPointRecord> for AirwayPointRecord {
    fn from(value: core::AirwayPointRecord) -> Self {
        Self {
            code: value.code,
            raw_code: value.raw_code,
            kind: value.kind,
            latitude: value.latitude,
            longitude: value.longitude,
        }
    }
}

impl From<core::AirwayRecord> for AirwayRecord {
    fn from(value: core::AirwayRecord) -> Self {
        Self {
            name: value.name,
            source: value.source,
            route_class: value.route_class,
            points: value.points.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<core_arcgis::ArcgisAirportRecord> for AirportRecord {
    fn from(value: core_arcgis::ArcgisAirportRecord) -> Self {
        Self {
            code: value.code,
            iata: value.iata,
            icao: value.icao,
            name: value.name,
            latitude: value.latitude,
            longitude: value.longitude,
            region: value.region,
            source: value.source,
        }
    }
}

impl From<core_arcgis::ArcgisNavpointRecord> for NavpointRecord {
    fn from(value: core_arcgis::ArcgisNavpointRecord) -> Self {
        Self {
            code: value.code,
            identifier: value.identifier,
            kind: value.kind,
            name: value.name,
            latitude: value.latitude,
            longitude: value.longitude,
            description: value.description,
            frequency: value.frequency,
            point_type: value.point_type,
            region: value.region,
            source: value.source,
        }
    }
}

impl From<core_arcgis::ArcgisAirwayPointRecord> for AirwayPointRecord {
    fn from(value: core_arcgis::ArcgisAirwayPointRecord) -> Self {
        Self {
            code: value.code,
            raw_code: value.raw_code,
            kind: value.kind,
            latitude: value.latitude,
            longitude: value.longitude,
        }
    }
}

impl From<core_arcgis::ArcgisAirwayRecord> for AirwayRecord {
    fn from(value: core_arcgis::ArcgisAirwayRecord) -> Self {
        Self {
            name: value.name,
            source: value.source,
            route_class: value.route_class,
            points: value.points.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<core_arcgis::ArcgisAirspaceRecord> for AirspaceRecord {
    fn from(value: core_arcgis::ArcgisAirspaceRecord) -> Self {
        Self {
            designator: value.designator,
            name: value.name,
            type_: value.type_,
            lower: value.lower,
            upper: value.upper,
            coordinates: value.coordinates,
            source: value.source,
        }
    }
}

impl From<core_nasr::NasrAirportRecord> for AirportRecord {
    fn from(value: core_nasr::NasrAirportRecord) -> Self {
        Self {
            code: value.code,
            iata: value.iata,
            icao: value.icao,
            name: value.name,
            latitude: value.latitude,
            longitude: value.longitude,
            region: value.region,
            source: value.source,
        }
    }
}

impl From<core_nasr::NasrNavpointRecord> for NavpointRecord {
    fn from(value: core_nasr::NasrNavpointRecord) -> Self {
        Self {
            code: value.code,
            identifier: value.identifier,
            kind: value.kind,
            name: value.name,
            latitude: value.latitude,
            longitude: value.longitude,
            description: value.description,
            frequency: value.frequency,
            point_type: value.point_type,
            region: value.region,
            source: value.source,
        }
    }
}

impl From<core_nasr::NasrAirwayPointRecord> for AirwayPointRecord {
    fn from(value: core_nasr::NasrAirwayPointRecord) -> Self {
        Self {
            code: value.code,
            raw_code: value.raw_code,
            kind: value.kind,
            latitude: value.latitude,
            longitude: value.longitude,
        }
    }
}

impl From<core_nasr::NasrAirwayRecord> for AirwayRecord {
    fn from(value: core_nasr::NasrAirwayRecord) -> Self {
        Self {
            name: value.name,
            source: value.source,
            route_class: value.route_class,
            points: value.points.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<core_nasr::NasrProcedureRecord> for ProcedureRecord {
    fn from(value: core_nasr::NasrProcedureRecord) -> Self {
        Self {
            name: value.name,
            source: value.source,
            procedure_kind: value.procedure_kind,
            route_class: value.route_class,
            airport: value.airport,
            points: value.points.into_iter().map(Into::into).collect(),
        }
    }
}

pub(crate) fn normalize_airway_name(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}
