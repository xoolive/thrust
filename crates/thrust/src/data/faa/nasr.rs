use crate::error::ThrustError;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "net")]
use std::fs;
use std::fs::File;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "net")]
use std::io::Write;
use std::io::{BufRead, BufReader, Cursor};
use std::path::{Path, PathBuf};
use zip::read::ZipArchive;

pub use crate::data::airac::{airac_code_from_date, effective_date_from_airac_code};

const NASR_BASE_URL: &str = "https://nfdc.faa.gov/webContent/28DaySub";

/// An AIRAC cycle representing a 28-day aeronautical information publication cycle.
///
/// AIRAC cycles are standardized worldwide and used to publish navigation data,
/// airport information, and procedures.
///
/// # Fields
/// * `code` - 4-character AIRAC code in format "YYCC" (e.g., "2508" for 2025 Cycle 08)
/// * `effective_date` - The date when this cycle becomes effective
#[derive(Debug, Clone)]
pub struct AiracCycle {
    pub code: String,
    pub effective_date: chrono::NaiveDate,
}

/// Summary information about a single file in an NASR dataset.
///
/// NASR (National Airspace System Resource) data is distributed as CSV files
/// within ZIP archives, one cycle per archive.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrFileSummary {
    /// Filename within the archive
    pub name: String,
    /// Uncompressed size in bytes
    pub size_bytes: u64,
    /// Compressed size in bytes
    pub compressed_size_bytes: u64,
    /// Number of lines in the CSV file (if detected)
    pub line_count: Option<u64>,
    /// Number of header columns (if detected)
    pub header_columns: Option<usize>,
    /// CSV delimiter character (if detected)
    pub delimiter: Option<String>,
}

/// Summary of all files in an NASR AIRAC cycle.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrCycleSummary {
    /// AIRAC code (e.g., "2508")
    pub airac_code: String,
    /// Effective date in YYYY-MM-DD format
    pub effective_date: String,
    /// Local path to the downloaded NASR ZIP file
    pub zip_path: String,
    /// List of files contained in the archive
    pub files: Vec<NasrFileSummary>,
}

/// A navigation point (waypoint, navaid, or fix) from FAA NASR data.
///
/// Points are referenced in routes, procedures, and airways.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrPoint {
    /// Unique identifier (e.g., "ORF", "RDBOE")
    pub identifier: String,
    /// Type of point: "NAVAID", "FIX", "AIRPORT", etc.
    pub kind: String,
    /// Latitude in decimal degrees
    pub latitude: f64,
    /// Longitude in decimal degrees
    pub longitude: f64,
    /// Name or description of the point
    pub name: Option<String>,
    /// Additional descriptive text
    pub description: Option<String>,
    /// VHF frequency (for navaids) in MHz
    pub frequency: Option<f64>,
    /// Sub-type classification
    pub point_type: Option<String>,
    /// ICAO region code
    pub region: Option<String>,
}

/// A segment of an ATS route (airway).
///
/// Airways consist of multiple segments, each defined by a "from" and "to" point.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrAirwaySegment {
    /// Full airway name (e.g., "J500")
    pub airway_name: String,
    /// Airway identifier number
    pub airway_id: String,
    /// Airway designation (letter prefix, e.g., "J")
    pub airway_designation: String,
    /// Location code for this segment
    pub airway_location: Option<String>,
    /// Starting point identifier
    pub from_point: String,
    /// Ending point identifier
    pub to_point: String,
}

/// An airspace boundary from FAA NASR data.
///
/// Airspaces are used for traffic management, approach control, and separation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrAirspace {
    /// Designator (e.g., "ORF A", "CHO C")
    pub designator: String,
    /// Name of the airspace
    pub name: Option<String>,
    /// Type (e.g., "Class A", "Class C", "TRSA")
    pub type_: Option<String>,
    /// Minimum altitude in feet (mean sea level)
    pub lower: Option<f64>,
    /// Maximum altitude in feet (mean sea level)
    pub upper: Option<f64>,
    /// Boundary polygon as (longitude, latitude) pairs
    pub coordinates: Vec<(f64, f64)>, // (lon, lat)
}

/// A leg (segment) of a SID or STAR procedure.
///
/// Procedures consist of multiple legs that guide aircraft along a defined path.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrProcedureLeg {
    /// Type of procedure: "SID" or "STAR"
    pub procedure_kind: String,
    /// Procedure identifier (e.g., "RCKT2")
    pub procedure_id: String,
    /// Route portion classification
    pub route_portion_type: String,
    /// Name of the route (optional)
    pub route_name: Option<String>,
    /// Body sequence number for this leg
    pub body_seq: Option<i32>,
    /// Sequence within the procedure
    pub point_seq: Option<i32>,
    /// Waypoint identifier for this leg
    pub point: String,
    /// Next waypoint (for multi-point procedures)
    pub next_point: Option<String>,
}

/// Complete Field15 navigation data for a single AIRAC cycle.
///
/// This is the result of parsing all navigation-related CSV files from NASR.
/// It contains waypoints, airways, and procedures that can be used to resolve
/// and enrich flight routes encoded in ICAO Field 15 format.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrField15Data {
    /// All navigation points (waypoints, navaids, fixes)
    pub points: Vec<NasrPoint>,
    /// All ATS route segments (airways)
    pub airways: Vec<NasrAirwaySegment>,
    /// SID procedure identifiers
    pub sid_designators: Vec<String>,
    /// STAR procedure identifiers
    pub star_designators: Vec<String>,
    /// All legs for SID procedures
    pub sid_legs: Vec<NasrProcedureLeg>,
    /// All legs for STAR procedures
    pub star_legs: Vec<NasrProcedureLeg>,
}

/// An index for quick lookup of Field15 elements by name.
///
/// This is used to speed up validation and enrichment of flight routes.
#[derive(Debug, Clone, Default)]
pub struct NasrField15Index {
    /// Set of point identifiers and names (uppercase)
    pub point_names: HashSet<String>,
    /// Set of airway names (uppercase)
    pub airway_names: HashSet<String>,
    /// Set of SID identifiers (uppercase)
    pub sid_names: HashSet<String>,
    /// Set of STAR identifiers (uppercase)
    pub star_names: HashSet<String>,
}

impl NasrField15Index {
    pub fn from_data(data: &NasrField15Data) -> Self {
        let mut idx = Self::default();

        for point in &data.points {
            if !point.identifier.is_empty() {
                idx.point_names.insert(point.identifier.to_uppercase());
            }
            if let Some(name) = &point.name {
                if !name.is_empty() {
                    idx.point_names.insert(name.to_uppercase());
                }
            }
        }

        for airway in &data.airways {
            if !airway.airway_name.is_empty() {
                idx.airway_names.insert(airway.airway_name.to_uppercase());
            }
            if !airway.airway_id.is_empty() {
                idx.airway_names.insert(airway.airway_id.to_uppercase());
            }
        }

        for sid in &data.sid_designators {
            idx.sid_names.insert(sid.to_uppercase());
        }
        for star in &data.star_designators {
            idx.star_names.insert(star.to_uppercase());
        }

        idx
    }
}

pub fn cycle_from_airac_code(airac_code: &str) -> Result<AiracCycle, ThrustError> {
    let effective_date = effective_date_from_airac_code(airac_code)?;
    Ok(AiracCycle {
        code: airac_code.to_string(),
        effective_date,
    })
}

pub fn nasr_zip_url_from_airac_code(airac_code: &str) -> Result<String, ThrustError> {
    let cycle = cycle_from_airac_code(airac_code)?;
    Ok(format!(
        "{NASR_BASE_URL}/28DaySubscription_Effective_{}.zip",
        cycle.effective_date.format("%Y-%m-%d")
    ))
}

pub fn download_nasr_zip_for_airac<P: AsRef<Path>>(airac_code: &str, output_dir: P) -> Result<PathBuf, ThrustError> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (airac_code, output_dir);
        return Err("download_nasr_zip_for_airac is not available on wasm; fetch in JS and pass bytes".into());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(not(feature = "net"))]
        {
            let _ = (airac_code, output_dir);
            Err("NASR download is disabled; enable feature 'net'".into())
        }

        #[cfg(feature = "net")]
        {
            let cycle = cycle_from_airac_code(airac_code)?;
            fs::create_dir_all(&output_dir)?;

            let filename = format!("NASR_{}_{}.zip", airac_code, cycle.effective_date.format("%Y-%m-%d"));
            let output_path = output_dir.as_ref().join(filename);

            if output_path.exists() {
                return Ok(output_path);
            }

            let url = nasr_zip_url_from_airac_code(airac_code)?;
            let bytes = reqwest::blocking::get(url)?.error_for_status()?.bytes()?;

            let mut file = File::create(&output_path)?;
            file.write_all(&bytes)?;

            Ok(output_path)
        }
    }
}

pub fn parse_nasr_zip_file<P: AsRef<Path>>(path: P) -> Result<Vec<NasrFileSummary>, ThrustError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut summaries = Vec::new();

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.is_dir() {
            continue;
        }

        let name = file.name().to_string();
        let size_bytes = file.size();
        let compressed_size_bytes = file.compressed_size();

        let mut summary = NasrFileSummary {
            name: name.clone(),
            size_bytes,
            compressed_size_bytes,
            line_count: None,
            header_columns: None,
            delimiter: None,
        };

        if is_text_like(&name) {
            let (line_count, header_columns, delimiter) = inspect_delimited_content(file)?;
            summary.line_count = Some(line_count);
            summary.header_columns = header_columns;
            summary.delimiter = delimiter.map(|c| c.to_string());
        }

        summaries.push(summary);
    }

    summaries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(summaries)
}

pub fn load_nasr_cycle_summary<P: AsRef<Path>>(
    airac_code: &str,
    output_dir: P,
) -> Result<NasrCycleSummary, ThrustError> {
    let cycle = cycle_from_airac_code(airac_code)?;
    let zip_path = download_nasr_zip_for_airac(airac_code, output_dir)?;
    let files = parse_nasr_zip_file(&zip_path)?;

    Ok(NasrCycleSummary {
        airac_code: cycle.code,
        effective_date: cycle.effective_date.to_string(),
        zip_path: zip_path.display().to_string(),
        files,
    })
}

pub fn parse_field15_data_from_nasr_zip<P: AsRef<Path>>(path: P) -> Result<NasrField15Data, ThrustError> {
    let mut csv_zip = open_csv_bundle(path)?;
    parse_field15_data_from_csv_bundle(&mut csv_zip)
}

pub fn parse_field15_data_from_nasr_bytes(bytes: &[u8]) -> Result<NasrField15Data, ThrustError> {
    let mut csv_zip = open_csv_bundle_from_bytes(bytes)?;
    parse_field15_data_from_csv_bundle(&mut csv_zip)
}

pub fn parse_airspaces_from_nasr_bytes(bytes: &[u8]) -> Result<Vec<NasrAirspace>, ThrustError> {
    let mut outer = ZipArchive::new(Cursor::new(bytes.to_vec()))?;
    let mut saa_bytes = Vec::new();

    {
        let mut saa_zip = outer.by_name("Additional_Data/AIXM/SAA-AIXM_5_Schema/SaaSubscriberFile.zip")?;
        std::io::copy(&mut saa_zip, &mut saa_bytes)?;
    }

    let mut level1 = ZipArchive::new(Cursor::new(saa_bytes))?;
    let mut sub_bytes = Vec::new();
    {
        let mut sub = level1.by_name("Saa_Sub_File.zip")?;
        std::io::copy(&mut sub, &mut sub_bytes)?;
    }

    let mut level2 = ZipArchive::new(Cursor::new(sub_bytes))?;
    let mut all = Vec::new();
    for i in 0..level2.len() {
        let mut entry = level2.by_index(i)?;
        if !entry.name().to_lowercase().ends_with(".xml") {
            continue;
        }
        let mut xml = Vec::new();
        std::io::copy(&mut entry, &mut xml)?;
        all.extend(parse_saa_xml_airspaces(&xml));
    }

    Ok(all)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrAirportRecord {
    pub code: String,
    pub iata: Option<String>,
    pub icao: Option<String>,
    pub name: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub region: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrNavpointRecord {
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrAirwayPointRecord {
    pub code: String,
    pub raw_code: String,
    pub kind: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrAirwayRecord {
    pub name: String,
    pub source: String,
    pub route_class: Option<String>,
    pub points: Vec<NasrAirwayPointRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrProcedureRecord {
    pub name: String,
    pub source: String,
    pub procedure_kind: String,
    pub route_class: Option<String>,
    pub airport: Option<String>,
    pub points: Vec<NasrAirwayPointRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrResolverData {
    pub airports: Vec<NasrAirportRecord>,
    pub navaids: Vec<NasrNavpointRecord>,
    pub airways: Vec<NasrAirwayRecord>,
    pub procedures: Vec<NasrProcedureRecord>,
    pub airspaces: Vec<NasrAirspace>,
}

pub fn parse_resolver_data_from_nasr_bytes(bytes: &[u8]) -> Result<NasrResolverData, ThrustError> {
    let data = parse_field15_data_from_nasr_bytes(bytes)?;
    let nasr_airspaces = parse_airspaces_from_nasr_bytes(bytes)?;

    let NasrField15Data {
        points,
        airways: airway_segments,
        sid_designators,
        star_designators,
        sid_legs,
        star_legs,
    } = data;

    let airports: Vec<NasrAirportRecord> = points
        .iter()
        .filter(|p| p.kind == "AIRPORT")
        .map(|p| {
            let code = p.identifier.to_uppercase();
            let iata = if code.len() == 3 { Some(code.clone()) } else { None };
            let icao = if code.len() == 4 { Some(code.clone()) } else { None };

            NasrAirportRecord {
                code,
                iata,
                icao,
                name: p.name.clone(),
                latitude: p.latitude,
                longitude: p.longitude,
                region: p.region.clone(),
                source: "faa_nasr".to_string(),
            }
        })
        .collect();

    let fixes: Vec<NasrNavpointRecord> = points
        .iter()
        .filter(|p| p.kind == "FIX")
        .map(|p| NasrNavpointRecord {
            code: normalize_point_code(&p.identifier),
            identifier: p.identifier.to_uppercase(),
            kind: "fix".to_string(),
            name: p.name.clone(),
            latitude: p.latitude,
            longitude: p.longitude,
            description: p.description.clone(),
            frequency: p.frequency,
            point_type: p.point_type.clone(),
            region: p.region.clone(),
            source: "faa_nasr".to_string(),
        })
        .collect();

    let mut navaids: Vec<NasrNavpointRecord> = points
        .iter()
        .filter(|p| p.kind == "NAVAID")
        .map(|p| NasrNavpointRecord {
            code: normalize_point_code(&p.identifier),
            identifier: p.identifier.to_uppercase(),
            kind: "navaid".to_string(),
            name: p.name.clone(),
            latitude: p.latitude,
            longitude: p.longitude,
            description: p.description.clone(),
            frequency: p.frequency,
            point_type: p.point_type.clone(),
            region: p.region.clone(),
            source: "faa_nasr".to_string(),
        })
        .collect();

    navaids.extend(fixes.iter().cloned());
    navaids.sort_by(|a, b| a.code.cmp(&b.code).then(a.point_type.cmp(&b.point_type)));
    navaids.dedup_by(|a, b| {
        a.code == b.code && a.point_type == b.point_type && a.latitude == b.latitude && a.longitude == b.longitude
    });

    let mut point_index: HashMap<String, NasrAirwayPointRecord> = HashMap::new();
    for p in &points {
        let normalized = normalize_point_code(&p.identifier);
        let record = NasrAirwayPointRecord {
            code: normalized.clone(),
            raw_code: p.identifier.to_uppercase(),
            kind: point_kind(&p.kind),
            latitude: p.latitude,
            longitude: p.longitude,
        };
        point_index.entry(p.identifier.to_uppercase()).or_insert(record.clone());
        point_index.entry(normalized).or_insert(record);
    }

    let mut grouped: HashMap<String, Vec<NasrAirwayPointRecord>> = HashMap::new();
    for seg in airway_segments {
        let route_name = if seg.airway_id.trim().is_empty() {
            seg.airway_name.clone()
        } else {
            seg.airway_id.clone()
        };
        let entry = grouped.entry(route_name).or_default();

        let from_key = seg.from_point.to_uppercase();
        let to_key = seg.to_point.to_uppercase();
        let from = point_index.get(&from_key).cloned().unwrap_or(NasrAirwayPointRecord {
            code: normalize_point_code(&from_key),
            raw_code: from_key.clone(),
            kind: "point".to_string(),
            latitude: 0.0,
            longitude: 0.0,
        });
        let to = point_index.get(&to_key).cloned().unwrap_or(NasrAirwayPointRecord {
            code: normalize_point_code(&to_key),
            raw_code: to_key.clone(),
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

    let airways: Vec<NasrAirwayRecord> = grouped
        .into_iter()
        .map(|(name, points)| NasrAirwayRecord {
            name,
            source: "faa_nasr".to_string(),
            route_class: None,
            points,
        })
        .collect();

    let procedures = build_procedure_records(&point_index, sid_designators, star_designators, sid_legs, star_legs);

    Ok(NasrResolverData {
        airports,
        navaids,
        airways,
        procedures,
        airspaces: nasr_airspaces,
    })
}

fn build_procedure_records(
    point_index: &HashMap<String, NasrAirwayPointRecord>,
    sid_designators: Vec<String>,
    star_designators: Vec<String>,
    sid_legs: Vec<NasrProcedureLeg>,
    star_legs: Vec<NasrProcedureLeg>,
) -> Vec<NasrProcedureRecord> {
    fn route_class_for(kind: &str) -> Option<String> {
        match kind {
            "SID" => Some("DP".to_string()),
            "STAR" => Some("AP".to_string()),
            _ => None,
        }
    }

    fn build_one(
        name: &str,
        kind: &str,
        legs: &[NasrProcedureLeg],
        point_index: &HashMap<String, NasrAirwayPointRecord>,
    ) -> NasrProcedureRecord {
        let mut sorted_legs = legs.to_vec();
        sorted_legs.sort_by_key(|leg| (leg.body_seq.unwrap_or(i32::MAX), leg.point_seq.unwrap_or(i32::MAX)));

        let mut ids: Vec<String> = Vec::new();
        for leg in &sorted_legs {
            let point = leg.point.trim().to_uppercase();
            if !point.is_empty() && ids.last() != Some(&point) {
                ids.push(point);
            }
            if let Some(next) = &leg.next_point {
                let next_id = next.trim().to_uppercase();
                if !next_id.is_empty() && ids.last() != Some(&next_id) {
                    ids.push(next_id);
                }
            }
        }

        let points = ids
            .into_iter()
            .filter_map(|id| {
                point_index.get(&id).cloned().or_else(|| {
                    let normalized = normalize_point_code(&id);
                    point_index.get(&normalized).cloned()
                })
            })
            .collect::<Vec<_>>();

        NasrProcedureRecord {
            name: name.to_uppercase(),
            source: "faa_nasr".to_string(),
            procedure_kind: kind.to_string(),
            route_class: route_class_for(kind),
            airport: None,
            points,
        }
    }

    let mut legs_by_id_kind: HashMap<(String, String), Vec<NasrProcedureLeg>> = HashMap::new();
    for leg in sid_legs.into_iter().chain(star_legs) {
        let id = leg.procedure_id.trim().to_uppercase();
        let kind = leg.procedure_kind.trim().to_uppercase();
        if id.is_empty() || kind.is_empty() {
            continue;
        }
        legs_by_id_kind.entry((id, kind)).or_default().push(leg);
    }

    let mut all_names: Vec<(String, String)> = sid_designators
        .into_iter()
        .map(|name| (name.trim().to_uppercase(), "SID".to_string()))
        .chain(
            star_designators
                .into_iter()
                .map(|name| (name.trim().to_uppercase(), "STAR".to_string())),
        )
        .collect();

    for (id, kind) in legs_by_id_kind.keys() {
        all_names.push((id.clone(), kind.clone()));
    }

    all_names.sort();
    all_names.dedup();

    all_names
        .into_iter()
        .map(|(name, kind)| {
            let legs = legs_by_id_kind
                .get(&(name.clone(), kind.clone()))
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            build_one(&name, &kind, legs, point_index)
        })
        .collect()
}

fn normalize_point_code(value: &str) -> String {
    value.split(':').next().unwrap_or(value).to_uppercase()
}

fn point_kind(kind: &str) -> String {
    match kind {
        "FIX" => "fix".to_string(),
        "NAVAID" => "navaid".to_string(),
        "AIRPORT" => "airport".to_string(),
        _ => "point".to_string(),
    }
}

fn parse_field15_data_from_csv_bundle(
    csv_zip: &mut ZipArchive<Cursor<Vec<u8>>>,
) -> Result<NasrField15Data, ThrustError> {
    let points = parse_points(csv_zip)?;
    let airways = parse_airways(csv_zip)?;
    let sid_designators = parse_designators(csv_zip, "DP_BASE.csv", &["DP_NAME", "DP_COMPUTER_CODE"])?;
    let star_designators = parse_designators(csv_zip, "STAR_BASE.csv", &["ARRIVAL_NAME", "STAR_COMPUTER_CODE"])?;
    let sid_legs = parse_procedure_legs(csv_zip, "DP_RTE.csv", "SID")?;
    let star_legs = parse_procedure_legs(csv_zip, "STAR_RTE.csv", "STAR")?;

    Ok(NasrField15Data {
        points,
        airways,
        sid_designators,
        star_designators,
        sid_legs,
        star_legs,
    })
}

fn open_csv_bundle_from_bytes(bytes: &[u8]) -> Result<ZipArchive<Cursor<Vec<u8>>>, ThrustError> {
    let mut outer = ZipArchive::new(Cursor::new(bytes.to_vec()))?;

    for i in 0..outer.len() {
        let mut entry = outer.by_index(i)?;
        let name = entry.name().to_string();
        if name.starts_with("CSV_Data/") && name.ends_with("_CSV.zip") {
            let mut inner_bytes = Vec::new();
            std::io::copy(&mut entry, &mut inner_bytes)?;
            return Ok(ZipArchive::new(Cursor::new(inner_bytes))?);
        }
    }

    Err("CSV bundle not found in NASR zip".into())
}

fn open_csv_bundle<P: AsRef<Path>>(path: P) -> Result<ZipArchive<Cursor<Vec<u8>>>, ThrustError> {
    let file = File::open(path)?;
    let mut outer = ZipArchive::new(file)?;

    for i in 0..outer.len() {
        let mut entry = outer.by_index(i)?;
        let name = entry.name().to_string();
        if name.starts_with("CSV_Data/") && name.ends_with("_CSV.zip") {
            let mut bytes = Vec::new();
            std::io::copy(&mut entry, &mut bytes)?;
            return Ok(ZipArchive::new(Cursor::new(bytes))?);
        }
    }

    Err("CSV bundle not found in NASR zip".into())
}

fn parse_saa_xml_airspaces(xml: &[u8]) -> Vec<NasrAirspace> {
    fn tag_is(tag: &[u8], suffix: &[u8]) -> bool {
        tag.ends_with(suffix)
    }

    fn read_text(reader: &mut Reader<Cursor<&[u8]>>, end: Vec<u8>) -> Option<String> {
        let mut out = String::new();
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Text(t)) => {
                    out.push_str(&String::from_utf8_lossy(t.as_ref()));
                }
                Ok(Event::CData(t)) => {
                    out.push_str(&String::from_utf8_lossy(t.as_ref()));
                }
                Ok(Event::End(e)) if e.name().as_ref() == end.as_slice() => break,
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
            buf.clear();
        }
        let txt = out.trim().to_string();
        if txt.is_empty() {
            None
        } else {
            Some(txt)
        }
    }

    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut in_airspace = false;
    let mut designator: Option<String> = None;
    let mut name: Option<String> = None;
    let mut type_: Option<String> = None;
    let mut lower: Option<f64> = None;
    let mut upper: Option<f64> = None;
    let mut coords: Vec<(f64, f64)> = Vec::new();
    let mut out = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = e.name().as_ref().to_vec();
                if tag_is(tag.as_slice(), b"Airspace") {
                    in_airspace = true;
                    designator = None;
                    name = None;
                    type_ = None;
                    lower = None;
                    upper = None;
                    coords.clear();
                } else if in_airspace && tag_is(tag.as_slice(), b"designator") {
                    designator = read_text(&mut reader, tag);
                } else if in_airspace && tag_is(tag.as_slice(), b"name") {
                    name = read_text(&mut reader, tag);
                } else if in_airspace && tag_is(tag.as_slice(), b"type") {
                    type_ = read_text(&mut reader, tag);
                } else if in_airspace && tag_is(tag.as_slice(), b"lowerLimit") {
                    lower = read_text(&mut reader, tag).and_then(|v| v.parse::<f64>().ok());
                } else if in_airspace && tag_is(tag.as_slice(), b"upperLimit") {
                    upper = read_text(&mut reader, tag).and_then(|v| v.parse::<f64>().ok());
                } else if in_airspace && tag_is(tag.as_slice(), b"pos") {
                    if let Some(text) = read_text(&mut reader, tag) {
                        let mut it = text.split_whitespace().filter_map(|x| x.parse::<f64>().ok());
                        if let (Some(lat), Some(lon)) = (it.next(), it.next()) {
                            coords.push((lon, lat));
                        }
                    }
                }
            }
            Ok(Event::End(e)) if tag_is(e.name().as_ref(), b"Airspace") && in_airspace => {
                let key = designator.clone().or_else(|| name.clone());
                if let Some(des) = key {
                    if coords.len() >= 3 {
                        out.push(NasrAirspace {
                            designator: des,
                            name: name.clone(),
                            type_: type_.clone(),
                            lower,
                            upper,
                            coordinates: coords.clone(),
                        });
                    }
                }
                in_airspace = false;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    out
}

fn parse_points(csv_zip: &mut ZipArchive<Cursor<Vec<u8>>>) -> Result<Vec<NasrPoint>, ThrustError> {
    let mut points = Vec::new();

    for row in read_csv_rows(csv_zip, "FIX_BASE.csv")? {
        if let (Some(id), Some(lat), Some(lon)) = (
            row.get("FIX_ID").map(|x| x.trim()).filter(|x| !x.is_empty()),
            parse_f64(row.get("LAT_DECIMAL")),
            parse_f64(row.get("LONG_DECIMAL")),
        ) {
            points.push(NasrPoint {
                identifier: id.to_string(),
                kind: "FIX".to_string(),
                latitude: lat,
                longitude: lon,
                name: Some(id.to_string()),
                description: None,
                frequency: None,
                point_type: row
                    .get("FIX_USE_CODE")
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty()),
                region: row
                    .get("ICAO_REGION_CODE")
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty()),
            });
        }
    }

    for row in read_csv_rows(csv_zip, "NAV_BASE.csv")? {
        if let (Some(id), Some(lat), Some(lon)) = (
            row.get("NAV_ID").map(|x| x.trim()).filter(|x| !x.is_empty()),
            parse_f64(row.get("LAT_DECIMAL")),
            parse_f64(row.get("LONG_DECIMAL")),
        ) {
            let nav_type = row.get("NAV_TYPE").map(|x| x.trim()).unwrap_or("");
            let base_name = row.get("NAME").map(|x| x.trim()).filter(|x| !x.is_empty());
            let city = row.get("CITY").map(|x| x.trim()).filter(|x| !x.is_empty());
            let description = match (base_name, city) {
                (Some(name), Some(city_name)) => {
                    Some(format!("{} {} {}", name, city_name, nav_type).trim().to_string())
                }
                (Some(name), None) => Some(format!("{} {}", name, nav_type).trim().to_string()),
                _ => None,
            };
            points.push(NasrPoint {
                identifier: format!("{}:{}", id, nav_type),
                kind: "NAVAID".to_string(),
                latitude: lat,
                longitude: lon,
                name: row.get("NAME").map(|x| x.trim().to_string()).filter(|x| !x.is_empty()),
                description,
                frequency: parse_f64(row.get("FREQ")),
                point_type: Some(nav_type.to_string()),
                region: row
                    .get("REGION_CODE")
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty()),
            });
        }
    }

    for row in read_csv_rows(csv_zip, "APT_BASE.csv")? {
        let lat = parse_f64(row.get("LAT_DECIMAL"));
        let lon = parse_f64(row.get("LONG_DECIMAL"));
        if let (Some(lat), Some(lon)) = (lat, lon) {
            for id in [row.get("ARPT_ID"), row.get("ICAO_ID")]
                .into_iter()
                .flatten()
                .map(|x| x.trim())
                .filter(|x| !x.is_empty())
            {
                points.push(NasrPoint {
                    identifier: id.to_string(),
                    kind: "AIRPORT".to_string(),
                    latitude: lat,
                    longitude: lon,
                    name: row
                        .get("ARPT_NAME")
                        .map(|x| x.trim().to_string())
                        .filter(|x| !x.is_empty()),
                    description: None,
                    frequency: None,
                    point_type: None,
                    region: row
                        .get("REGION_CODE")
                        .map(|x| x.trim().to_string())
                        .filter(|x| !x.is_empty()),
                });
            }
        }
    }

    Ok(points)
}

fn parse_airways(csv_zip: &mut ZipArchive<Cursor<Vec<u8>>>) -> Result<Vec<NasrAirwaySegment>, ThrustError> {
    let mut segments = Vec::new();

    for row in read_csv_rows(csv_zip, "AWY_BASE.csv")? {
        let airway_id = row.get("AWY_ID").map(|x| x.trim()).unwrap_or("");
        let airway_designation = row.get("AWY_DESIGNATION").map(|x| x.trim()).unwrap_or("");
        let airway_string = row.get("AIRWAY_STRING").map(|x| x.trim()).unwrap_or("");
        if airway_id.is_empty() || airway_string.is_empty() {
            continue;
        }

        let points = airway_string
            .split_whitespace()
            .map(|x| x.trim())
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>();

        for window in points.windows(2) {
            if let [from, to] = window {
                segments.push(NasrAirwaySegment {
                    airway_name: build_airway_name(airway_designation, airway_id),
                    airway_id: airway_id.to_string(),
                    airway_designation: airway_designation.to_string(),
                    airway_location: row
                        .get("AWY_LOCATION")
                        .map(|x| x.trim().to_string())
                        .filter(|x| !x.is_empty()),
                    from_point: (*from).to_string(),
                    to_point: (*to).to_string(),
                });
            }
        }
    }

    Ok(segments)
}

fn parse_designators(
    csv_zip: &mut ZipArchive<Cursor<Vec<u8>>>,
    filename: &str,
    fields: &[&str],
) -> Result<Vec<String>, ThrustError> {
    let mut set = HashSet::new();
    for row in read_csv_rows(csv_zip, filename)? {
        for field in fields {
            if let Some(value) = row.get(*field).map(|x| x.trim()).filter(|x| !x.is_empty()) {
                for token in normalize_designator_candidates(value) {
                    set.insert(token);
                }
            }
        }
    }
    let mut out = set.into_iter().collect::<Vec<_>>();
    out.sort();
    Ok(out)
}

fn normalize_designator_candidates(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return out;
    }

    out.push(trimmed.to_string());

    let before_dot = trimmed.split('.').next().unwrap_or(trimmed).trim();
    if !before_dot.is_empty() {
        out.push(before_dot.to_string());
    }

    let compact = before_dot
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>();
    if !compact.is_empty() {
        out.push(compact);
    }

    out.sort();
    out.dedup();
    out
}

fn build_airway_name(designation: &str, airway_id: &str) -> String {
    if designation.is_empty() {
        return airway_id.to_string();
    }
    if airway_id.starts_with(designation) {
        airway_id.to_string()
    } else {
        format!("{}{}", designation, airway_id)
    }
}

fn parse_procedure_legs(
    csv_zip: &mut ZipArchive<Cursor<Vec<u8>>>,
    filename: &str,
    kind: &str,
) -> Result<Vec<NasrProcedureLeg>, ThrustError> {
    let rows = read_csv_rows(csv_zip, filename)?;
    let mut legs = rows
        .into_iter()
        .filter_map(|row| {
            let point = row.get("POINT").map(|x| x.trim().to_string()).unwrap_or_default();
            if point.is_empty() {
                return None;
            }

            let procedure_id = row
                .get("DP_COMPUTER_CODE")
                .or_else(|| row.get("STAR_COMPUTER_CODE"))
                .map(|x| x.trim().to_string())
                .unwrap_or_default();

            if procedure_id.is_empty() {
                return None;
            }

            Some(NasrProcedureLeg {
                procedure_kind: kind.to_string(),
                procedure_id,
                route_portion_type: row
                    .get("ROUTE_PORTION_TYPE")
                    .map(|x| x.trim().to_string())
                    .unwrap_or_default(),
                route_name: row
                    .get("ROUTE_NAME")
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty()),
                body_seq: parse_i32(row.get("BODY_SEQ")),
                point_seq: parse_i32(row.get("POINT_SEQ")),
                point,
                next_point: row
                    .get("NEXT_POINT")
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty()),
            })
        })
        .collect::<Vec<_>>();

    legs.sort_by_key(|leg| {
        (
            leg.procedure_id.clone(),
            leg.body_seq.unwrap_or(0),
            leg.point_seq.unwrap_or(0),
        )
    });
    Ok(legs)
}

fn read_csv_rows(
    csv_zip: &mut ZipArchive<Cursor<Vec<u8>>>,
    filename: &str,
) -> Result<Vec<HashMap<String, String>>, ThrustError> {
    let file = csv_zip.by_name(filename)?;
    let mut rdr = csv::ReaderBuilder::new().has_headers(true).from_reader(file);
    let mut rows = Vec::new();

    let headers = rdr
        .byte_headers()?
        .iter()
        .map(|h| String::from_utf8_lossy(h).trim().to_string())
        .collect::<Vec<_>>();

    for record in rdr.byte_records() {
        let record = record?;
        let mut row = HashMap::new();

        for (idx, key) in headers.iter().enumerate() {
            let value = record
                .get(idx)
                .map(|v| String::from_utf8_lossy(v).trim().to_string())
                .unwrap_or_default();
            row.insert(key.clone(), value);
        }

        rows.push(row);
    }

    Ok(rows)
}

fn parse_f64(value: Option<&String>) -> Option<f64> {
    value.and_then(|x| x.trim().parse::<f64>().ok())
}

fn parse_i32(value: Option<&String>) -> Option<i32> {
    value.and_then(|x| x.trim().parse::<i32>().ok())
}

fn is_text_like(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".csv")
        || lower.ends_with(".txt")
        || lower.ends_with(".dat")
        || lower.ends_with(".xml")
        || lower.ends_with(".json")
}

fn detect_delimiter(header: &str) -> Option<char> {
    let candidates = [',', '|', '\t', ';'];
    candidates
        .into_iter()
        .max_by_key(|c| header.matches(*c).count())
        .filter(|c| header.matches(*c).count() > 0)
}

type DelimitedContentInfo = (u64, Option<usize>, Option<char>);

fn inspect_delimited_content<R: std::io::Read>(file: R) -> Result<DelimitedContentInfo, ThrustError> {
    let mut reader = BufReader::new(file);
    let mut first_line_bytes = Vec::new();
    let mut line_count = 0u64;

    if reader.read_until(b'\n', &mut first_line_bytes)? > 0 {
        line_count = 1;
    }

    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        let bytes = reader.read_until(b'\n', &mut buffer)?;
        if bytes == 0 {
            break;
        }
        line_count += 1;
    }

    let first_line = String::from_utf8_lossy(&first_line_bytes).trim_end().to_string();
    let delimiter = detect_delimiter(&first_line);
    let header_columns = delimiter.map(|d| first_line.split(d).count());
    Ok((line_count, header_columns, delimiter))
}

#[cfg(test)]
mod tests {
    use super::{build_procedure_records, read_csv_rows, NasrAirwayPointRecord, NasrProcedureLeg};
    use crate::data::field15::{Connector, Field15Element, Field15Parser};
    use std::collections::HashMap;
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    #[test]
    fn read_csv_rows_tolerates_invalid_utf8() {
        let mut inner_buf = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut inner_buf);
            writer
                .start_file("APT_BASE.csv", SimpleFileOptions::default())
                .expect("cannot start csv entry");
            writer
                .write_all(b"ICAO_ID,ARPT_ID,LAT_DECIMAL,LONG_DECIMAL,ARPT_NAME,REGION_CODE\n")
                .expect("cannot write header");
            writer
                .write_all(b"KLAX,LAX,33.94,-118.40,LOS\xFFANGELES,US\n")
                .expect("cannot write row");
            writer.finish().expect("cannot finish zip");
        }

        let mut archive =
            zip::read::ZipArchive::new(Cursor::new(inner_buf.into_inner())).expect("cannot open in-memory zip");
        let rows = read_csv_rows(&mut archive, "APT_BASE.csv").expect("csv parse failed");

        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.get("ICAO_ID").map(String::as_str), Some("KLAX"));
        assert_eq!(row.get("ARPT_ID").map(String::as_str), Some("LAX"));
        let name = row.get("ARPT_NAME").expect("missing airport name");
        assert!(name.starts_with("LOS"));
        assert!(name.contains('�'));
    }

    #[test]
    fn build_procedure_records_includes_sid_and_star_examples_from_notebook() {
        let mut point_index = HashMap::new();
        point_index.insert(
            "FISTO".to_string(),
            NasrAirwayPointRecord {
                code: "FISTO".to_string(),
                raw_code: "FISTO".to_string(),
                kind: "fix".to_string(),
                latitude: 43.60,
                longitude: 1.20,
            },
        );
        point_index.insert(
            "KEPER".to_string(),
            NasrAirwayPointRecord {
                code: "KEPER".to_string(),
                raw_code: "KEPER".to_string(),
                kind: "fix".to_string(),
                latitude: 49.50,
                longitude: 2.30,
            },
        );

        let sid_legs = vec![NasrProcedureLeg {
            procedure_kind: "SID".to_string(),
            procedure_id: "FISTO5A".to_string(),
            route_portion_type: "COMMON".to_string(),
            route_name: None,
            body_seq: Some(1),
            point_seq: Some(1),
            point: "FISTO".to_string(),
            next_point: None,
        }];

        let star_legs = vec![NasrProcedureLeg {
            procedure_kind: "STAR".to_string(),
            procedure_id: "KEPER9E".to_string(),
            route_portion_type: "COMMON".to_string(),
            route_name: None,
            body_seq: Some(1),
            point_seq: Some(1),
            point: "KEPER".to_string(),
            next_point: None,
        }];

        let procedures = build_procedure_records(
            &point_index,
            vec!["FISTO5A".to_string()],
            vec!["KEPER9E".to_string()],
            sid_legs,
            star_legs,
        );

        let sid = procedures
            .iter()
            .find(|p| p.name == "FISTO5A")
            .expect("missing SID FISTO5A");
        assert_eq!(sid.procedure_kind, "SID");
        assert_eq!(sid.route_class.as_deref(), Some("DP"));
        assert_eq!(sid.points.first().map(|p| p.code.as_str()), Some("FISTO"));

        let star = procedures
            .iter()
            .find(|p| p.name == "KEPER9E")
            .expect("missing STAR KEPER9E");
        assert_eq!(star.procedure_kind, "STAR");
        assert_eq!(star.route_class.as_deref(), Some("AP"));
        assert_eq!(star.points.first().map(|p| p.code.as_str()), Some("KEPER"));
    }

    #[test]
    fn field15_parser_detects_sid_and_star_for_notebook_route() {
        let field15 = "N0430F300 FISTO6B FISTO DCT POI DCT PEPAX UT182 NIMER/N0401F240 UT182 KEPER KEPER9E";
        let elements = Field15Parser::parse(field15);

        let has_sid = elements
            .iter()
            .any(|e| matches!(e, Field15Element::Connector(Connector::Sid(name)) if name == "FISTO6B"));
        let has_star = elements
            .iter()
            .any(|e| matches!(e, Field15Element::Connector(Connector::Star(name)) if name == "KEPER9E"));

        assert!(has_sid, "expected SID FISTO6B in parsed elements");
        assert!(has_star, "expected STAR KEPER9E in parsed elements");
    }
}
