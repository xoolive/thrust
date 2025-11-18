use polars::prelude::*;
use std::{collections::HashMap, env, path::Path};
use thrust::data::eurocontrol::aixm::designated_point::parse_designated_point_zip_file;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path_to_directory>", args[0]);
        std::process::exit(1);
    }
    let path = Path::new(&args[1]);
    let path = path.join("DesignatedPoint.BASELINE.zip");

    match parse_designated_point_zip_file(path) {
        Ok(points) => {
            if let Ok(df) = df!(
                "identifier" => points.values().map(|point| point.identifier.clone()).collect::<Vec<_>>(),
                "designator" => points.values().map(|point| point.designator.clone()).collect::<Vec<_>>(),
                "name" => points.values().map(|point| point.name.clone()).collect::<Vec<_>>(),
                "latitude" => points.values().map(|point| point.latitude).collect::<Vec<_>>(),
                "longitude" => points.values().map(|point| point.longitude).collect::<Vec<_>>(),
                "type" => points.values().map(|point| point.r#type.clone()).collect::<Vec<_>>(),
            ) {
                println!("{df:?}");
            }
        }
        Err(e) => eprintln!("Error parsing designated point file: {e}"),
    }
}
