use std::collections::HashSet;
use std::env;
use std::path::Path;

use thrust::data::eurocontrol::aixm::airport_heliport::parse_airport_heliport_zip_file;
use thrust::data::eurocontrol::aixm::designated_point::parse_designated_point_zip_file;
use thrust::data::eurocontrol::aixm::navaid::parse_navaid_zip_file;
use thrust::data::eurocontrol::aixm::route::parse_route_zip_file;
use thrust::data::eurocontrol::aixm::standard_instrument_arrival::parse_standard_instrument_arrival_zip_file;
use thrust::data::eurocontrol::aixm::standard_instrument_departure::parse_standard_instrument_departure_zip_file;
use thrust::data::eurocontrol::ddr::navpoints::parse_navpoints_dir as parse_ddr_navpoints_dir;
use thrust::data::eurocontrol::ddr::procedures::{
    parse_sid_star_dir as parse_ddr_sid_star_dir, procedure_designator_index,
};
use thrust::data::eurocontrol::ddr::routes::parse_routes_dir as parse_ddr_routes_dir;
use thrust::data::faa::nasr::{parse_field15_data_from_nasr_zip, NasrField15Index};
use thrust::data::faa::nat::{fetch_nat_bulletin, resolve_named_points_with_nasr};
use thrust::data::field15::{Connector, Field15Element, Field15Parser, Point};

const DEFAULT_FLIGHTPLAN: &str = "N0490F360 ELCOB6B ELCOB UT300 SENLO UN502 JSY DCT LIZAD DCT MOPAT DCT LUNIG DCT MOMIN DCT PIKIL/M084F380 NATD HOIST/N0490F380 N756C ANATI/N0441F340 DCT MIVAX DCT OBTEK DCT XORLO ROCKT2";

#[derive(Default)]
struct EurocontrolField15Index {
    point_names: HashSet<String>,
    airway_names: HashSet<String>,
    sid_names: HashSet<String>,
    star_names: HashSet<String>,
}

#[derive(Default)]
struct DdrField15Index {
    point_names: HashSet<String>,
    airway_names: HashSet<String>,
    sid_names: HashSet<String>,
    star_names: HashSet<String>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 || args.len() > 5 {
        eprintln!(
            "Usage: {} <path_to_eurocontrol_airac_dir> <path_to_faa_nasr_zip> [path_to_ddr_dir] [flightplan]",
            args[0]
        );
        std::process::exit(1);
    }

    let eurocontrol_path = Path::new(&args[1]);
    let nasr_zip = Path::new(&args[2]);

    let (ddr_path, flightplan) = match args.len() {
        3 => (None, DEFAULT_FLIGHTPLAN),
        4 => {
            let p = Path::new(&args[3]);
            if p.exists() {
                (Some(p), DEFAULT_FLIGHTPLAN)
            } else {
                (None, args[3].as_str())
            }
        }
        _ => (Some(Path::new(&args[3])), args[4].as_str()),
    };

    let elements = Field15Parser::parse(flightplan);
    println!("Field15: {flightplan}");
    println!("Parsed elements: {}", elements.len());

    let euro_index = build_eurocontrol_index(eurocontrol_path);
    let ddr_index = ddr_path.map(build_ddr_index).transpose();
    let faa_data = parse_field15_data_from_nasr_zip(nasr_zip);

    if let (Ok(euro_index), Ok(faa_data), Ok(ddr_index)) = (euro_index, faa_data, ddr_index) {
        let faa_index = NasrField15Index::from_data(&faa_data);
        let nat_tracks = fetch_nat_bulletin()
            .ok()
            .map(|mut bulletin| {
                let _ = resolve_named_points_with_nasr(&mut bulletin, &faa_data.points);
                bulletin
                    .tracks
                    .into_iter()
                    .map(|track| track.track_id.to_uppercase())
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();

        let euro = coverage_report("Eurocontrol only", &elements, |kind, token| match kind {
            "point" => euro_index.point_names.contains(token),
            "airway" => euro_index.airway_names.contains(token),
            "sid" => euro_index.sid_names.contains(token),
            "star" => euro_index.star_names.contains(token),
            _ => false,
        });

        let faa = coverage_report("FAA NASR only", &elements, |kind, token| match kind {
            "point" => faa_index.point_names.contains(token),
            "airway" => faa_index.airway_names.contains(token),
            "sid" => faa_index.sid_names.contains(token),
            "star" => faa_index.star_names.contains(token),
            _ => false,
        });

        if let Some(ddr_index) = &ddr_index {
            let _ = coverage_report("Eurocontrol DDR only", &elements, |kind, token| match kind {
                "point" => ddr_index.point_names.contains(token),
                "airway" => ddr_index.airway_names.contains(token),
                "sid" => ddr_index.sid_names.contains(token),
                "star" => ddr_index.star_names.contains(token),
                "nat" => false,
                _ => false,
            });
        }

        let combined = coverage_report(
            "Combined strategy (EU first, FAA fallback)",
            &elements,
            |kind, token| match kind {
                "airway" => {
                    euro_index.airway_names.contains(token)
                        || ddr_index.as_ref().is_some_and(|idx| idx.airway_names.contains(token))
                        || faa_index.airway_names.contains(token)
                }
                "sid" => {
                    euro_index.sid_names.contains(token)
                        || ddr_index.as_ref().is_some_and(|idx| idx.sid_names.contains(token))
                        || faa_index.sid_names.contains(token)
                }
                "star" => {
                    euro_index.star_names.contains(token)
                        || ddr_index.as_ref().is_some_and(|idx| idx.star_names.contains(token))
                        || faa_index.star_names.contains(token)
                }
                "point" => {
                    euro_index.point_names.contains(token)
                        || ddr_index.as_ref().is_some_and(|idx| idx.point_names.contains(token))
                        || faa_index.point_names.contains(token)
                }
                "nat" => nat_tracks.contains(&nat_track_id(token)),
                _ => false,
            },
        );

        println!("\nObservations:");
        println!(
            "- Eurocontrol unresolved critical tokens: {}",
            euro.unresolved.join(", ")
        );
        println!("- FAA unresolved critical tokens: {}", faa.unresolved.join(", "));
        if let Some(ddr_idx) = &ddr_index {
            println!(
                "- DDR index loaded: {} points, {} airway names, {} SID refs, {} STAR refs",
                ddr_idx.point_names.len(),
                ddr_idx.airway_names.len(),
                ddr_idx.sid_names.len(),
                ddr_idx.star_names.len()
            );
        }
        println!(
            "- Combined unresolved critical tokens: {}",
            combined.unresolved.join(", ")
        );
        println!(
            "- NAT tracks considered: {}",
            if nat_tracks.is_empty() {
                "unavailable (fetch failed)".to_string()
            } else {
                format!("{} active track IDs", nat_tracks.len())
            }
        );
    } else {
        eprintln!("Error building indexes from provided datasets");
    }
}

struct Report {
    unresolved: Vec<String>,
}

fn coverage_report<F>(label: &str, elements: &[Field15Element], mut matcher: F) -> Report
where
    F: FnMut(&str, &String) -> bool,
{
    let mut checked = 0usize;
    let mut matched = 0usize;
    let mut unresolved = Vec::new();

    for element in elements {
        match element {
            Field15Element::Point(Point::Waypoint(name)) | Field15Element::Point(Point::Aerodrome(name)) => {
                let t = name.to_uppercase();
                checked += 1;
                if matcher("point", &t) {
                    matched += 1;
                } else {
                    unresolved.push(t);
                }
            }
            Field15Element::Connector(Connector::Airway(name)) => {
                let t = name.to_uppercase();
                checked += 1;
                if matcher("airway", &t) {
                    matched += 1;
                } else {
                    unresolved.push(t);
                }
            }
            Field15Element::Connector(Connector::Sid(name)) => {
                let t = name.to_uppercase();
                checked += 1;
                if matcher("sid", &t) {
                    matched += 1;
                } else {
                    unresolved.push(t);
                }
            }
            Field15Element::Connector(Connector::Star(name)) => {
                let t = name.to_uppercase();
                checked += 1;
                if matcher("star", &t) {
                    matched += 1;
                } else {
                    unresolved.push(t);
                }
            }
            Field15Element::Connector(Connector::Nat(name)) => {
                let t = name.to_uppercase();
                checked += 1;
                if matcher("nat", &t) {
                    matched += 1;
                } else {
                    unresolved.push(format!("{}(track)", t));
                }
            }
            _ => {}
        }
    }

    unresolved.sort();
    unresolved.dedup();

    println!("\n{label}:");
    println!("- matched {matched}/{checked} critical tokens");
    if !unresolved.is_empty() {
        println!("- unresolved: {}", unresolved.join(", "));
    }

    Report { unresolved }
}

fn build_eurocontrol_index(path: &Path) -> Result<EurocontrolField15Index, Box<dyn std::error::Error>> {
    let points = parse_designated_point_zip_file(path.join("DesignatedPoint.BASELINE.zip"))?;
    let navaids = parse_navaid_zip_file(path.join("Navaid.BASELINE.zip"))?;
    let airports = parse_airport_heliport_zip_file(path.join("AirportHeliport.BASELINE.zip"))?;
    let routes = parse_route_zip_file(path.join("Route.BASELINE.zip"))?;
    let sids = parse_standard_instrument_departure_zip_file(path.join("StandardInstrumentDeparture.BASELINE.zip"))?;
    let stars = parse_standard_instrument_arrival_zip_file(path.join("StandardInstrumentArrival.BASELINE.zip"))?;

    let mut index = EurocontrolField15Index::default();

    for p in points.values() {
        index.point_names.insert(p.designator.to_uppercase());
    }
    for n in navaids.values() {
        if let Some(name) = &n.name {
            index.point_names.insert(name.to_uppercase());
        }
    }
    for a in airports.values() {
        index.point_names.insert(a.icao.to_uppercase());
    }

    for r in routes.values() {
        if let Some(name) = route_name(r) {
            index.airway_names.insert(name.to_uppercase());
        }
    }

    for sid in sids.values() {
        index.sid_names.insert(sid.designator.to_uppercase());
    }
    for star in stars.values() {
        index.star_names.insert(star.designator.to_uppercase());
    }

    Ok(index)
}

fn route_name(route: &thrust::data::eurocontrol::aixm::route::Route) -> Option<String> {
    match (&route.second_letter, &route.number) {
        (Some(second), Some(number)) => {
            let mut name = String::new();
            if let Some(prefix) = &route.prefix {
                name.push_str(prefix);
            }
            name.push_str(second);
            name.push_str(number);
            if let Some(multi) = &route.multiple_identifier {
                name.push_str(multi);
            }
            Some(name)
        }
        _ => None,
    }
}

fn build_ddr_index(path: &Path) -> Result<DdrField15Index, Box<dyn std::error::Error>> {
    let navpoints = parse_ddr_navpoints_dir(path)?;
    let routes = parse_ddr_routes_dir(path)?;
    let (sids, stars) = parse_ddr_sid_star_dir(path)?;

    let mut index = DdrField15Index::default();
    for p in &navpoints {
        index.point_names.insert(p.name.to_uppercase());
    }
    for r in &routes {
        index.airway_names.insert(r.route.to_uppercase());
    }
    index.sid_names = procedure_designator_index(&sids);
    index.star_names = procedure_designator_index(&stars);

    Ok(index)
}

fn nat_track_id(token: &str) -> String {
    let upper = token.to_uppercase();
    if let Some(rest) = upper.strip_prefix("NAT") {
        rest.to_string()
    } else {
        upper
    }
}
