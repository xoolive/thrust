use polars::prelude::*;
use std::{env, path::Path};
use thrust::data::eurocontrol::aixm::airspace::parse_airspace_zip_file;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path_to_directory>", args[0]);
        std::process::exit(1);
    }
    let path = Path::new(&args[1]).join("Airspace.BASELINE.zip");

    match parse_airspace_zip_file(path) {
        Ok(airspaces) => {
            if let Ok(df) = df!(
                "identifier" => airspaces.values().map(|x| x.identifier.clone()).collect::<Vec<_>>(),
                "designator" => airspaces.values().map(|x| x.designator.clone()).collect::<Vec<_>>(),
                "type" => airspaces.values().map(|x| x.type_.clone()).collect::<Vec<_>>(),
                "name" => airspaces.values().map(|x| x.name.clone()).collect::<Vec<_>>(),
                "volumes" => airspaces.values().map(|x| x.volumes.len() as u32).collect::<Vec<_>>(),
            ) {
                println!("{df:?}");
            }
        }
        Err(e) => eprintln!("Error parsing airspace file: {e}"),
    }
}
