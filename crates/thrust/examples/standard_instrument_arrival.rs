use polars::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::{env, path::Path};
use thrust::data::eurocontrol::aixm::airport_heliport::{parse_airport_heliport_zip_file, AirportHeliport};
use thrust::data::eurocontrol::aixm::arrival_leg::{parse_arrival_leg_zip_file, ArrivalLeg};
use thrust::data::eurocontrol::aixm::designated_point::{parse_designated_point_zip_file, DesignatedPoint};
use thrust::data::eurocontrol::aixm::navaid::{parse_navaid_zip_file, Navaid};
use thrust::data::eurocontrol::aixm::route_segment::PointReference;
use thrust::data::eurocontrol::aixm::standard_instrument_arrival::{
    parse_standard_instrument_arrival_zip_file, StandardInstrumentArrival,
};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || args.len() > 3 {
        eprintln!("Usage: {} <path_to_directory> [star_designator]", args[0]);
        std::process::exit(1);
    }
    let path = Path::new(&args[1]);
    let requested_star = args.get(2).map(|s| s.trim().to_string());

    let arrival_path = path.join("StandardInstrumentArrival.BASELINE.zip");
    let airport_path = path.join("AirportHeliport.BASELINE.zip");
    let designated_point_path = path.join("DesignatedPoint.BASELINE.zip");
    let navaid_path = path.join("Navaid.BASELINE.zip");
    let arrival_leg_path = path.join("ArrivalLeg.BASELINE.zip");

    match (
        parse_standard_instrument_arrival_zip_file(arrival_path),
        parse_airport_heliport_zip_file(airport_path),
        parse_designated_point_zip_file(designated_point_path),
        parse_navaid_zip_file(navaid_path),
        parse_arrival_leg_zip_file(arrival_leg_path),
    ) {
        (Ok(arrivals), Ok(airports), Ok(points), Ok(navaids), Ok(arrival_legs)) => {
            if let Some(designator) = requested_star {
                print_star_details(&designator, &arrivals, &arrival_legs, &airports, &points, &navaids);
            } else {
                print_star_dataframe(&arrivals, &arrival_legs, &airports, &points, &navaids);
            }
        }
        _ => eprintln!("Error parsing standard instrument arrival dependencies"),
    }
}

fn print_star_dataframe(
    arrivals: &HashMap<String, StandardInstrumentArrival>,
    arrival_legs: &HashMap<String, ArrivalLeg>,
    airports: &HashMap<String, AirportHeliport>,
    points: &HashMap<String, DesignatedPoint>,
    navaids: &HashMap<String, Navaid>,
) {
    let mut leg_points_by_arrival = HashMap::<String, Vec<PointReference>>::new();
    for leg in arrival_legs.values() {
        if let Some(arrival_id) = &leg.arrival {
            let entry = leg_points_by_arrival.entry(arrival_id.clone()).or_default();
            entry.push(leg.start.clone());
            entry.push(leg.end.clone());
        }
    }

    if let Ok(df) = df!(
        "identifier" => arrivals.values().map(|arrival| arrival.identifier.clone()).collect::<Vec<_>>(),
        "designator" => arrivals.values().map(|arrival| arrival.designator.clone()).collect::<Vec<_>>(),
        "airport_heliport" => arrivals.values().map(|arrival| {
            arrival
                .airport_heliport
                .as_ref()
                .and_then(|id| airports.get(id).map(|airport| format!("{} ({})", airport.icao, airport.name)))
        }).collect::<Vec<_>>(),
        "instruction" => arrivals.values().map(|arrival| arrival.instruction.clone()).collect::<Vec<_>>(),
        "connecting_points" => arrivals
            .values()
            .map(|arrival| {
                let source_points: Vec<PointReference> = leg_points_by_arrival
                    .get(&arrival.identifier)
                    .cloned()
                    .unwrap_or_else(|| arrival.connecting_points.clone());

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

fn print_star_details(
    designator: &str,
    arrivals: &HashMap<String, StandardInstrumentArrival>,
    arrival_legs: &HashMap<String, ArrivalLeg>,
    airports: &HashMap<String, AirportHeliport>,
    points: &HashMap<String, DesignatedPoint>,
    navaids: &HashMap<String, Navaid>,
) {
    let matching = arrivals
        .values()
        .filter(|star| star.designator.eq_ignore_ascii_case(designator))
        .collect::<Vec<_>>();

    if matching.is_empty() {
        eprintln!("No STAR found for designator: {designator}");
        return;
    }

    for star in matching {
        let airport = star
            .airport_heliport
            .as_ref()
            .and_then(|id| airports.get(id).map(|a| format!("{} ({})", a.icao, a.name)))
            .unwrap_or_else(|| "Unknown airport".to_string());
        println!("STAR {} | {} | {}", star.designator, star.identifier, airport);

        let legs = arrival_legs
            .values()
            .filter(|leg| leg.arrival.as_ref().is_some_and(|id| id == &star.identifier))
            .map(|leg| (leg.start.clone(), leg.end.clone()))
            .collect::<Vec<_>>();

        let ordered_points = if legs.is_empty() {
            star.connecting_points.clone()
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
