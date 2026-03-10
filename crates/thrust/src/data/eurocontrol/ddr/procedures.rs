use crate::error::ThrustError;

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// A reference to an instrument procedure (SID or STAR) from DDR data.
///
/// Represents a Standard Instrument Departure (SID) or Standard Arrival Route (STAR)
/// procedure with minimal data: airport, procedure name, and type.
///
/// # Fields
/// - `airport`: Departure/arrival airport ICAO code (e.g., "KSEA")
/// - `designator`: Published procedure name (e.g., "KSEA01", "ORCAS3")
/// - `kind`: Procedure type ("SID" or "STAR")
/// - `raw`: Raw procedure definition string (format varies by source)
///
/// # Example
/// ```ignore
/// let proc = DdrProcedureRef {
///     airport: "KSEA".to_string(),
///     designator: "KSEA01".to_string(),
///     kind: "SID".to_string(),
///     raw: "KSEA KSEA01 SID ...".to_string(),
/// };
/// ```
///
/// # Note
/// For detailed procedure information (legs, waypoints, restrictions),
/// use [`StandardInstrumentDeparture`](crate::data::eurocontrol::aixm::standard_instrument_departure::StandardInstrumentDeparture)
/// or [`StandardInstrumentArrival`](crate::data::eurocontrol::aixm::standard_instrument_arrival::StandardInstrumentArrival) from AIXM data.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DdrProcedureRef {
    pub airport: String,
    pub designator: String,
    pub kind: String,
    pub raw: String,
}

pub fn parse_sid_star_dir<P: AsRef<Path>>(dir: P) -> Result<(Vec<DdrProcedureRef>, Vec<DdrProcedureRef>), ThrustError> {
    let mut sids = Vec::new();
    let mut stars = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|x| x.to_str()) else {
            continue;
        };
        if name.ends_with(".sid") {
            sids.extend(parse_procedure_file(&path, "SID")?);
        } else if name.ends_with(".star") {
            stars.extend(parse_procedure_file(&path, "STAR")?);
        }
    }

    Ok((sids, stars))
}

pub fn parse_procedure_file<P: AsRef<Path>>(path: P, kind: &str) -> Result<Vec<DdrProcedureRef>, ThrustError> {
    let text = std::fs::read_to_string(path)?;
    let mut rows = Vec::new();

    for line in text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        let tokens = line.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 2 {
            continue;
        }
        let airport = tokens[0].to_string();
        let designator = tokens[1].to_string();
        rows.push(DdrProcedureRef {
            airport,
            designator,
            kind: kind.to_string(),
            raw: line.to_string(),
        });
    }

    Ok(rows)
}

pub fn procedure_designator_index(procedures: &[DdrProcedureRef]) -> HashSet<String> {
    let mut out = HashSet::new();
    for p in procedures {
        for c in normalize_designator_candidates(&p.designator) {
            out.insert(c);
        }
    }
    out
}

fn normalize_designator_candidates(designator: &str) -> Vec<String> {
    let mut out = vec![designator.to_uppercase()];
    let upper = designator.to_uppercase();

    if let Some(base) = upper.strip_suffix(".D") {
        out.push(base.to_string());
    }
    if let Some(base) = upper.strip_suffix(".A") {
        out.push(base.to_string());
    }
    if let Some(base) = upper.strip_suffix('.') {
        out.push(base.to_string());
    }

    let compact = upper.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
    if !compact.is_empty() {
        out.push(compact);
    }

    out.sort();
    out.dedup();
    out
}
