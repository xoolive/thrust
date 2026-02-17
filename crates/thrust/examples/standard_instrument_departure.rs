use polars::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::{env, path::Path};
use thrust::data::eurocontrol::aixm::airport_heliport::{parse_airport_heliport_zip_file, AirportHeliport};
use thrust::data::eurocontrol::aixm::departure_leg::{parse_departure_leg_zip_file, DepartureLeg};
use thrust::data::eurocontrol::aixm::designated_point::{parse_designated_point_zip_file, DesignatedPoint};
use thrust::data::eurocontrol::aixm::navaid::{parse_navaid_zip_file, Navaid};
use thrust::data::eurocontrol::aixm::route_segment::PointReference;
use thrust::data::eurocontrol::aixm::standard_instrument_departure::{
    parse_standard_instrument_departure_zip_file, StandardInstrumentDeparture,
};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || args.len() > 3 {
        eprintln!("Usage: {} <path_to_directory> [sid_designator]", args[0]);
        std::process::exit(1);
    }
    let path = Path::new(&args[1]);
    let requested_sid = args.get(2).map(|s| s.trim().to_string());

    let departure_path = path.join("StandardInstrumentDeparture.BASELINE.zip");
    let airport_path = path.join("AirportHeliport.BASELINE.zip");
    let designated_point_path = path.join("DesignatedPoint.BASELINE.zip");
    let navaid_path = path.join("Navaid.BASELINE.zip");
    let departure_leg_path = path.join("DepartureLeg.BASELINE.zip");

    match (
        parse_standard_instrument_departure_zip_file(departure_path),
        parse_airport_heliport_zip_file(airport_path),
        parse_designated_point_zip_file(designated_point_path),
        parse_navaid_zip_file(navaid_path),
        parse_departure_leg_zip_file(departure_leg_path),
    ) {
        (Ok(departures), Ok(airports), Ok(points), Ok(navaids), Ok(departure_legs)) => {
            if let Some(designator) = requested_sid {
                print_sid_details(&designator, &departures, &departure_legs, &airports, &points, &navaids);
            } else {
                print_sid_dataframe(&departures, &departure_legs, &airports, &points, &navaids);
            }
        }
        _ => eprintln!("Error parsing standard instrument departure dependencies"),
    }
}

fn print_sid_dataframe(
    departures: &HashMap<String, StandardInstrumentDeparture>,
    departure_legs: &HashMap<String, DepartureLeg>,
    airports: &HashMap<String, AirportHeliport>,
    points: &HashMap<String, DesignatedPoint>,
    navaids: &HashMap<String, Navaid>,
) {
    let mut leg_points_by_departure = HashMap::<String, Vec<PointReference>>::new();
    for leg in departure_legs.values() {
        if let Some(departure_id) = &leg.departure {
            let entry = leg_points_by_departure.entry(departure_id.clone()).or_default();
            entry.push(leg.start.clone());
            entry.push(leg.end.clone());
        }
    }

    if let Ok(df) = df!(
        "identifier" => departures.values().map(|departure| departure.identifier.clone()).collect::<Vec<_>>(),
        "designator" => departures.values().map(|departure| departure.designator.clone()).collect::<Vec<_>>(),
        "airport_heliport" => departures.values().map(|departure| {
            departure
                .airport_heliport
                .as_ref()
                .and_then(|id| airports.get(id).map(|airport| format!("{} ({})", airport.icao, airport.name)))
        }).collect::<Vec<_>>(),
        "instruction" => departures.values().map(|departure| departure.instruction.clone()).collect::<Vec<_>>(),
        "connecting_points" => departures
            .values()
            .map(|departure| {
                let source_points: Vec<PointReference> = leg_points_by_departure
                    .get(&departure.identifier)
                    .cloned()
                    .unwrap_or_else(|| departure.connecting_points.clone());

                let mut named_points = source_points
                    .iter()
                    .map(|point_ref| resolve_point_name(point_ref, airports, points, navaids))
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>();

                named_points.sort();
                named_points.dedup();
                named_points.join(",")
            })
            .collect::<Vec<_>>(),
    ) {
        println!("{df:?}");
    }
}

fn print_sid_details(
    designator: &str,
    departures: &HashMap<String, StandardInstrumentDeparture>,
    departure_legs: &HashMap<String, DepartureLeg>,
    airports: &HashMap<String, AirportHeliport>,
    points: &HashMap<String, DesignatedPoint>,
    navaids: &HashMap<String, Navaid>,
) {
    let matching = departures
        .values()
        .filter(|sid| sid.designator.eq_ignore_ascii_case(designator))
        .collect::<Vec<_>>();

    if matching.is_empty() {
        eprintln!("No SID found for designator: {designator}");
        return;
    }

    for sid in matching {
        let airport = sid
            .airport_heliport
            .as_ref()
            .and_then(|id| airports.get(id).map(|a| format!("{} ({})", a.icao, a.name)))
            .unwrap_or_else(|| "Unknown airport".to_string());
        println!("SID {} | {} | {}", sid.designator, sid.identifier, airport);

        let legs = departure_legs
            .values()
            .filter(|leg| leg.departure.as_ref().is_some_and(|id| id == &sid.identifier))
            .map(|leg| (leg.start.clone(), leg.end.clone()))
            .collect::<Vec<_>>();

        let ordered_points = if legs.is_empty() {
            sid.connecting_points.clone()
        } else {
            order_points_from_legs(&legs)
        };

        for (idx, point_ref) in ordered_points.iter().enumerate() {
            if let Some((kind, name, lat, lon)) = resolve_point_detail(point_ref, airports, points, navaids) {
                println!("{:03} {} {} {:.6} {:.6}", idx + 1, kind, name, lat, lon);
            }
        }
        println!();
    }
}

fn order_points_from_legs(legs: &[(PointReference, PointReference)]) -> Vec<PointReference> {
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    let mut indegree: HashMap<String, usize> = HashMap::new();
    let mut refs: HashMap<String, PointReference> = HashMap::new();

    for (start, end) in legs {
        let s = start.name();
        let e = end.name();
        if s.is_empty() || e.is_empty() {
            continue;
        }
        refs.entry(s.clone()).or_insert_with(|| start.clone());
        refs.entry(e.clone()).or_insert_with(|| end.clone());
        adjacency.entry(s.clone()).or_default().push(e.clone());
        indegree.entry(s).or_insert(0);
        *indegree.entry(e).or_insert(0) += 1;
    }

    let mut starts = indegree
        .iter()
        .filter_map(|(k, v)| if *v == 0 { Some(k.clone()) } else { None })
        .collect::<Vec<_>>();
    starts.sort();

    let mut queue = VecDeque::from(starts);
    let mut seen = HashSet::new();
    let mut ordered_names = Vec::new();

    while let Some(node) = queue.pop_front() {
        if !seen.insert(node.clone()) {
            continue;
        }
        ordered_names.push(node.clone());
        for next in adjacency.get(&node).cloned().unwrap_or_default() {
            if let Some(v) = indegree.get_mut(&next) {
                *v = v.saturating_sub(1);
                if *v == 0 {
                    queue.push_back(next);
                }
            }
        }
    }

    let mut remaining = refs.keys().filter(|k| !seen.contains(*k)).cloned().collect::<Vec<_>>();
    remaining.sort();
    ordered_names.extend(remaining);

    ordered_names
        .into_iter()
        .filter_map(|name| refs.get(&name).cloned())
        .collect()
}

fn resolve_point_name(
    point_ref: &PointReference,
    airports: &HashMap<String, AirportHeliport>,
    points: &HashMap<String, DesignatedPoint>,
    navaids: &HashMap<String, Navaid>,
) -> String {
    match point_ref {
        PointReference::DesignatedPoint(id) => points
            .get(id)
            .map(|point| point.designator.clone())
            .unwrap_or_else(|| id.clone()),
        PointReference::Navaid(id) => navaids
            .get(id)
            .and_then(|navaid| navaid.name.clone())
            .unwrap_or_else(|| id.clone()),
        PointReference::AirportHeliport(id) => airports
            .get(id)
            .map(|airport| airport.icao.clone())
            .unwrap_or_else(|| id.clone()),
        PointReference::None => String::new(),
    }
}

fn resolve_point_detail(
    point_ref: &PointReference,
    airports: &HashMap<String, AirportHeliport>,
    points: &HashMap<String, DesignatedPoint>,
    navaids: &HashMap<String, Navaid>,
) -> Option<(&'static str, String, f64, f64)> {
    match point_ref {
        PointReference::DesignatedPoint(id) => points
            .get(id)
            .map(|p| ("FIX", p.designator.clone(), p.latitude, p.longitude)),
        PointReference::Navaid(id) => navaids.get(id).map(|n| {
            (
                "NAVAID",
                n.name.clone().unwrap_or_else(|| id.clone()),
                n.latitude,
                n.longitude,
            )
        }),
        PointReference::AirportHeliport(id) => airports
            .get(id)
            .map(|a| ("AIRPORT", a.icao.clone(), a.latitude, a.longitude)),
        PointReference::None => None,
    }
}
