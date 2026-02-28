use dotenvy::dotenv;
use reqwest::blocking::Client;
use serde_json::Value;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use thrust::data::airac::effective_date_from_airac_code;
use thrust::data::eurocontrol::aixm::airport_heliport::parse_airport_heliport_zip_file;
use thrust::data::eurocontrol::aixm::designated_point::parse_designated_point_zip_file;
use thrust::data::eurocontrol::aixm::navaid::parse_navaid_zip_file;
use thrust::data::eurocontrol::database::{AirwayDatabase, ResolvedRoute};
use thrust::data::eurocontrol::ddr::navpoints::parse_navpoints_path;
use thrust::data::eurocontrol::ddr::routes::parse_routes_path;
use thrust::data::faa::nasr::parse_field15_data_from_nasr_bytes;

const FAA_ARCGIS_BASE: &str = "https://opendata.arcgis.com/datasets";
const FAA_ARCGIS_AIRPORTS_DATASET: &str = "e747ab91a11045e8b3f8a3efd093d3b5_0";
const FAA_ARCGIS_DESIGNATED_POINTS_DATASET: &str = "861043a88ff4486c97c3789e7dcdccc6_0";
const FAA_ARCGIS_NAVAID_COMPONENTS_DATASET: &str = "c9254c171b6741d3a5e494860761443a_0";
const FAA_ARCGIS_ATS_ROUTES_DATASET: &str = "acf64966af5f48a1a40fdbcb31238ba7_0";
const FAA_NASR_BASE: &str = "https://nfdc.faa.gov/webContent/28DaySub";

fn maybe_load_dotenv() {
    let _ = dotenv();
}

fn cache_root() -> PathBuf {
    env::var("FAA_TEST_DATA_DIR").map(PathBuf::from).unwrap_or_else(|_| {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".cache").join("thrust-faa")
    })
}

fn fetch_url_to_path(url: &str, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() && path.metadata()?.len() > 0 {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent("thrust-tests/0.1")
        .build()?;
    let mut last_error: Option<reqwest::Error> = None;
    for attempt in 0..5 {
        match client.get(url).send().and_then(|resp| resp.error_for_status()) {
            Ok(resp) => {
                let body = resp.bytes()?;
                fs::write(path, &body)?;
                return Ok(());
            }
            Err(err) => {
                last_error = Some(err);
                if attempt < 4 {
                    thread::sleep(Duration::from_secs(2_u64.pow((attempt as u32).min(4))));
                }
            }
        }
    }

    match last_error {
        Some(err) => Err(Box::new(err)),
        None => Err("request failed without an error".into()),
    }
}

fn ensure_arcgis_geojson(filename: &str, dataset_id: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = cache_root().join("arcgis").join(filename);
    let url = format!("{FAA_ARCGIS_BASE}/{dataset_id}.geojson");
    fetch_url_to_path(&url, &path)?;
    Ok(path)
}

fn ensure_nasr_zip() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(explicit) = env::var("FAA_NASR_ZIP") {
        let p = PathBuf::from(explicit);
        if p.exists() && p.metadata()?.len() > 0 {
            return Ok(p);
        }
    }

    let nasr_root = cache_root().join("nasr");
    if nasr_root.exists() {
        let mut named_files = Vec::new();
        for entry in fs::read_dir(&nasr_root)? {
            let entry = match entry {
                Ok(v) => v,
                Err(_) => continue,
            };
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            if name.starts_with("28DaySubscription_Effective_")
                && name.ends_with(".zip")
                && path.metadata().map(|m| m.len() > 0).unwrap_or(false)
            {
                named_files.push(path);
            }
        }
        named_files.sort();
        if let Some(path) = named_files.pop() {
            return Ok(path);
        }
    }

    if let Ok(code) = env::var("FAA_NASR_AIRAC") {
        if !code.is_empty() {
            if let Ok(date) = effective_date_from_airac_code(&code) {
                let expected = nasr_root.join(format!("28DaySubscription_Effective_{}.zip", date.format("%Y-%m-%d")));
                if expected.exists() && expected.metadata()?.len() > 0 {
                    return Ok(expected);
                }
            }
        }
    }

    let mut candidates = Vec::new();
    if let Ok(code) = env::var("FAA_NASR_AIRAC") {
        if !code.is_empty() {
            candidates.push(code);
        }
    }
    for year in [26_i32, 25_i32, 24_i32] {
        let yy = year.rem_euclid(100);
        for cycle in (1..=13).rev() {
            candidates.push(format!("{yy:02}{cycle:02}"));
        }
    }

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent("thrust-tests/0.1")
        .build()?;

    for code in candidates {
        let date = match effective_date_from_airac_code(&code) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let url = format!(
            "{FAA_NASR_BASE}/28DaySubscription_Effective_{}.zip",
            date.format("%Y-%m-%d")
        );
        let response = client.get(&url).send();
        if let Ok(resp) = response {
            if resp.status().is_success() {
                let target = nasr_root.join(format!("28DaySubscription_Effective_{}.zip", date.format("%Y-%m-%d")));
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&target, resp.bytes()?)?;
                return Ok(target);
            }
        }
    }

    Err("unable to prepare NASR zip for tests".into())
}

#[test]
fn eurocontrol_entities_are_resolvable_when_paths_are_set() {
    maybe_load_dotenv();
    let aixm_path = match env::var("THRUST_AIXM_PATH") {
        Ok(v) => PathBuf::from(v),
        Err(_) => return,
    };
    if !aixm_path.exists() {
        return;
    }

    let airports = parse_airport_heliport_zip_file(aixm_path.join("AirportHeliport.BASELINE.zip"))
        .expect("unable to parse AIXM airports");
    let airport_codes = airports.values().map(|a| a.icao.to_uppercase()).collect::<HashSet<_>>();
    for code in ["EHAM", "LSZH", "LFCL", "LFCX"] {
        assert!(airport_codes.contains(code), "missing AIXM airport {code}");
    }
    assert!(
        airports
            .values()
            .any(|a| a.icao.eq_ignore_ascii_case("EHAM") && a.name.to_uppercase().contains("SCHIPHOL")),
        "EHAM airport name does not include SCHIPHOL"
    );
    assert!(
        airports
            .values()
            .any(|a| a.icao.eq_ignore_ascii_case("LSZH") && a.name.to_uppercase().contains("ZURICH")),
        "LSZH airport name does not include ZURICH"
    );

    let fixes = parse_designated_point_zip_file(aixm_path.join("DesignatedPoint.BASELINE.zip"))
        .expect("unable to parse AIXM fixes");
    let fix_codes = fixes
        .values()
        .map(|p| p.designator.to_uppercase())
        .collect::<HashSet<_>>();
    assert!(fix_codes.contains("NARAK"), "missing AIXM fix NARAK");

    let navaids = parse_navaid_zip_file(aixm_path.join("Navaid.BASELINE.zip")).expect("unable to parse AIXM navaids");
    let navaid_codes = navaids
        .values()
        .filter_map(|n| n.name.clone())
        .map(|s| s.to_uppercase())
        .collect::<HashSet<_>>();
    assert!(navaid_codes.contains("GAI"), "missing AIXM navaid GAI");
    assert!(navaid_codes.contains("TOU"), "missing AIXM navaid TOU");

    let db = AirwayDatabase::new(&aixm_path).expect("unable to build AIXM airway db");
    let routes = ResolvedRoute::lookup("UM605", &db);
    assert!(!routes.is_empty(), "missing AIXM route UM605");

    if let Ok(ddr_raw) = env::var("THRUST_DDR_PATH") {
        let ddr_path = PathBuf::from(ddr_raw);
        if ddr_path.exists() {
            let ddr_navpoints = parse_navpoints_path(&ddr_path).expect("unable to parse DDR navpoints");
            let ddr_routes = parse_routes_path(&ddr_path).expect("unable to parse DDR routes");
            assert!(
                ddr_navpoints.iter().any(|p| p.name.eq_ignore_ascii_case("NARAK")),
                "missing DDR navpoint NARAK"
            );
            assert!(
                ddr_routes.iter().any(|r| r.route.eq_ignore_ascii_case("UM605")),
                "missing DDR route UM605"
            );
        }
    }
}

#[test]
fn faa_arcgis_entities_are_present() {
    maybe_load_dotenv();
    let airports_path = ensure_arcgis_geojson("faa_airports.json", FAA_ARCGIS_AIRPORTS_DATASET)
        .expect("unable to fetch FAA airports geojson");
    let airports_json = fs::read_to_string(airports_path).expect("unable to read FAA airports json");
    let payload: Value = serde_json::from_str(&airports_json).expect("invalid FAA airports json");
    let features = payload
        .get("features")
        .and_then(Value::as_array)
        .expect("FAA airports missing features");
    let airport_codes = features
        .iter()
        .filter_map(|f| f.get("properties"))
        .filter_map(|p| p.get("ICAO_ID").or_else(|| p.get("IDENT")))
        .filter_map(Value::as_str)
        .map(|s| s.to_uppercase())
        .collect::<HashSet<_>>();
    for code in ["KLAX", "KATL", "KJFK", "KORD", "CYVR", "CYUL"] {
        assert!(airport_codes.contains(code), "missing FAA airport {code}");
    }

    let designated_points_path =
        ensure_arcgis_geojson("faa_designated_points.json", FAA_ARCGIS_DESIGNATED_POINTS_DATASET)
            .expect("unable to fetch FAA designated points");
    let designated_points_payload: Value = serde_json::from_str(
        &fs::read_to_string(designated_points_path).expect("unable to read FAA designated points json"),
    )
    .expect("invalid FAA designated points json");
    let designated_points = designated_points_payload
        .get("features")
        .and_then(Value::as_array)
        .expect("FAA designated points missing features");
    assert!(
        designated_points
            .iter()
            .filter_map(|f| f.get("properties"))
            .any(|p| p.get("IDENT").and_then(Value::as_str) == Some("BASYE")),
        "missing FAA fix BASYE"
    );

    let navaids_path = ensure_arcgis_geojson("faa_navaid_components.json", FAA_ARCGIS_NAVAID_COMPONENTS_DATASET)
        .expect("unable to fetch FAA navaid components");
    let navaids_payload: Value =
        serde_json::from_str(&fs::read_to_string(navaids_path).expect("unable to read FAA navaids json"))
            .expect("invalid FAA navaids json");
    let navaids = navaids_payload
        .get("features")
        .and_then(Value::as_array)
        .expect("FAA navaids missing features");
    assert!(
        navaids
            .iter()
            .filter_map(|f| f.get("properties"))
            .any(|p| p.get("IDENT").and_then(Value::as_str) == Some("BAF")),
        "missing FAA navaid BAF"
    );
    assert!(
        navaids.iter().any(|f| {
            f.get("properties").and_then(|p| p.get("IDENT")).and_then(Value::as_str) == Some("BAF")
                && f.get("properties")
                    .and_then(|p| p.get("NAME"))
                    .and_then(Value::as_str)
                    .map(|s| s.to_uppercase().contains("BARNES"))
                    .unwrap_or(false)
        }),
        "missing FAA navaid BAF with expected name"
    );

    let routes_path = ensure_arcgis_geojson("faa_ats_routes.json", FAA_ARCGIS_ATS_ROUTES_DATASET)
        .expect("unable to fetch FAA ATS routes");
    let routes_payload: Value =
        serde_json::from_str(&fs::read_to_string(routes_path).expect("unable to read FAA routes json"))
            .expect("invalid FAA routes json");
    let routes = routes_payload
        .get("features")
        .and_then(Value::as_array)
        .expect("FAA routes missing features");
    assert!(
        routes
            .iter()
            .filter_map(|f| f.get("properties"))
            .any(|p| p.get("IDENT").and_then(Value::as_str) == Some("J48")),
        "missing FAA route J48"
    );
}

#[test]
fn faa_nasr_entities_are_present() {
    maybe_load_dotenv();
    let nasr_zip = ensure_nasr_zip().expect("unable to fetch NASR zip");
    let bytes = fs::read(nasr_zip).expect("unable to read NASR zip bytes");
    let parsed = parse_field15_data_from_nasr_bytes(&bytes).expect("unable to parse NASR field15 data");

    let airport_codes = parsed
        .points
        .iter()
        .filter(|p| p.kind == "AIRPORT")
        .map(|p| p.identifier.to_uppercase())
        .collect::<HashSet<_>>();
    for code in ["KLAX", "KATL", "KJFK", "KORD"] {
        assert!(airport_codes.contains(code), "missing NASR airport {code}");
    }
    assert!(
        parsed.points.iter().any(|p| {
            p.kind == "AIRPORT"
                && p.identifier.eq_ignore_ascii_case("KLAX")
                && p.name
                    .as_deref()
                    .map(|name| name.to_uppercase().contains("LOS ANGELES"))
                    .unwrap_or(false)
        }),
        "missing NASR KLAX airport with expected name"
    );

    assert!(
        parsed
            .points
            .iter()
            .any(|p| p.kind == "FIX" && p.identifier.eq_ignore_ascii_case("BASYE")),
        "missing NASR fix BASYE"
    );

    assert!(
        parsed.points.iter().any(|p| {
            p.kind == "NAVAID"
                && p.identifier.to_uppercase().starts_with("BAF:")
                && p.name
                    .as_deref()
                    .map(|name| name.to_uppercase().contains("BARNES"))
                    .unwrap_or(false)
                && p.point_type.as_deref().unwrap_or("").to_uppercase().contains("VOR")
        }),
        "missing NASR navaid BAF VOR/DME"
    );

    assert!(
        parsed.airways.iter().any(|a| a.airway_name.eq_ignore_ascii_case("J48")),
        "missing NASR route J48"
    );
}
