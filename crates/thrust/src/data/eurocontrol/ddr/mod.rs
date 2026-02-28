pub mod airspaces;
pub mod airports;
pub mod freeroute;
pub mod navpoints;
pub mod procedures;
pub mod routes;

use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::read::ZipArchive;

pub(crate) fn file_name_matches(entry_name: &str, prefix: &str, suffix: &str) -> bool {
    Path::new(entry_name)
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|name| name.starts_with(prefix) && name.ends_with(suffix))
}

pub(crate) fn read_first_zip_entry_bytes<P: AsRef<Path>>(
    zip_path: P,
    predicate: impl Fn(&str) -> bool,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    for idx in 0..archive.len() {
        let mut entry = archive.by_index(idx)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if predicate(&name) {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            return Ok(bytes);
        }
    }
    Err("matching file not found in zip archive".into())
}
