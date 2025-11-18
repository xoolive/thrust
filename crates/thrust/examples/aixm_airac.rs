use std::{env, path::Path};
use thrust::data::eurocontrol::database::{AirwayDatabase, ResolvedPoint, ResolvedRoute};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <path_to_directory> <navaid_or_airway_name>", args[0]);
        std::process::exit(1);
    }
    let path = Path::new(&args[1]);
    let name = &args[2];

    let db = AirwayDatabase::new(path)?;

    let candidates = ResolvedPoint::lookup(name, &db);
    for candidate in candidates {
        println!("Found point: {:?}", candidate);
    }

    let candidates = ResolvedRoute::lookup(name, &db);
    for candidate in candidates {
        println!("Found route: {:?}", candidate);
    }

    Ok(())
}
