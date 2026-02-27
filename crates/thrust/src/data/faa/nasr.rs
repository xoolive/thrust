use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use std::fs::File;
#[cfg(not(target_arch = "wasm32"))]
use std::io::Write;
use std::io::{BufRead, BufReader, Cursor};
use std::path::{Path, PathBuf};
use zip::read::ZipArchive;

pub use crate::data::airac::{airac_code_from_date, effective_date_from_airac_code};

const NASR_BASE_URL: &str = "https://nfdc.faa.gov/webContent/28DaySub";

#[derive(Debug, Clone)]
pub struct AiracCycle {
    pub code: String,
    pub effective_date: chrono::NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrFileSummary {
    pub name: String,
    pub size_bytes: u64,
    pub compressed_size_bytes: u64,
    pub line_count: Option<u64>,
    pub header_columns: Option<usize>,
    pub delimiter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrCycleSummary {
    pub airac_code: String,
    pub effective_date: String,
    pub zip_path: String,
    pub files: Vec<NasrFileSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrPoint {
    pub identifier: String,
    pub kind: String,
    pub latitude: f64,
    pub longitude: f64,
    pub name: Option<String>,
    pub description: Option<String>,
    pub frequency: Option<f64>,
    pub point_type: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrAirwaySegment {
    pub airway_name: String,
    pub airway_id: String,
    pub airway_designation: String,
    pub airway_location: Option<String>,
    pub from_point: String,
    pub to_point: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrAirspace {
    pub designator: String,
    pub name: Option<String>,
    pub type_: Option<String>,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub coordinates: Vec<(f64, f64)>, // (lon, lat)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrProcedureLeg {
    pub procedure_kind: String,
    pub procedure_id: String,
    pub route_portion_type: String,
    pub route_name: Option<String>,
    pub body_seq: Option<i32>,
    pub point_seq: Option<i32>,
    pub point: String,
    pub next_point: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NasrField15Data {
    pub points: Vec<NasrPoint>,
    pub airways: Vec<NasrAirwaySegment>,
    pub sid_designators: Vec<String>,
    pub star_designators: Vec<String>,
    pub sid_legs: Vec<NasrProcedureLeg>,
    pub star_legs: Vec<NasrProcedureLeg>,
}

#[derive(Debug, Clone, Default)]
pub struct NasrField15Index {
    pub point_names: HashSet<String>,
    pub airway_names: HashSet<String>,
    pub sid_names: HashSet<String>,
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

pub fn cycle_from_airac_code(airac_code: &str) -> Result<AiracCycle, Box<dyn std::error::Error>> {
    let effective_date = effective_date_from_airac_code(airac_code)?;
    Ok(AiracCycle {
        code: airac_code.to_string(),
        effective_date,
    })
}

pub fn nasr_zip_url_from_airac_code(airac_code: &str) -> Result<String, Box<dyn std::error::Error>> {
    let cycle = cycle_from_airac_code(airac_code)?;
    Ok(format!(
        "{NASR_BASE_URL}/28DaySubscription_Effective_{}.zip",
        cycle.effective_date.format("%Y-%m-%d")
    ))
}

pub fn download_nasr_zip_for_airac<P: AsRef<Path>>(
    airac_code: &str,
    output_dir: P,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (airac_code, output_dir);
        return Err("download_nasr_zip_for_airac is not available on wasm; fetch in JS and pass bytes".into());
    }

    #[cfg(not(target_arch = "wasm32"))]
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

pub fn parse_nasr_zip_file<P: AsRef<Path>>(path: P) -> Result<Vec<NasrFileSummary>, Box<dyn std::error::Error>> {
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
) -> Result<NasrCycleSummary, Box<dyn std::error::Error>> {
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

pub fn parse_field15_data_from_nasr_zip<P: AsRef<Path>>(
    path: P,
) -> Result<NasrField15Data, Box<dyn std::error::Error>> {
    let mut csv_zip = open_csv_bundle(path)?;
    parse_field15_data_from_csv_bundle(&mut csv_zip)
}

pub fn parse_field15_data_from_nasr_bytes(bytes: &[u8]) -> Result<NasrField15Data, Box<dyn std::error::Error>> {
    let mut csv_zip = open_csv_bundle_from_bytes(bytes)?;
    parse_field15_data_from_csv_bundle(&mut csv_zip)
}

pub fn parse_airspaces_from_nasr_bytes(bytes: &[u8]) -> Result<Vec<NasrAirspace>, Box<dyn std::error::Error>> {
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

fn parse_field15_data_from_csv_bundle(
    csv_zip: &mut ZipArchive<Cursor<Vec<u8>>>,
) -> Result<NasrField15Data, Box<dyn std::error::Error>> {
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

fn open_csv_bundle_from_bytes(bytes: &[u8]) -> Result<ZipArchive<Cursor<Vec<u8>>>, Box<dyn std::error::Error>> {
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

fn open_csv_bundle<P: AsRef<Path>>(path: P) -> Result<ZipArchive<Cursor<Vec<u8>>>, Box<dyn std::error::Error>> {
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
            Ok(Event::End(e)) => {
                if tag_is(e.name().as_ref(), b"Airspace") && in_airspace {
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
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    out
}

fn parse_points(csv_zip: &mut ZipArchive<Cursor<Vec<u8>>>) -> Result<Vec<NasrPoint>, Box<dyn std::error::Error>> {
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

fn parse_airways(
    csv_zip: &mut ZipArchive<Cursor<Vec<u8>>>,
) -> Result<Vec<NasrAirwaySegment>, Box<dyn std::error::Error>> {
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
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
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
) -> Result<Vec<NasrProcedureLeg>, Box<dyn std::error::Error>> {
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
) -> Result<Vec<HashMap<String, String>>, Box<dyn std::error::Error>> {
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

fn inspect_delimited_content<R: std::io::Read>(file: R) -> Result<DelimitedContentInfo, Box<dyn std::error::Error>> {
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
    use super::read_csv_rows;
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
}
