use polars::prelude::*;
use std::{env, path::Path};
use thrust::data::eurocontrol::aixm::route::parse_route_zip_file;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path_to_directory>", args[0]);
        std::process::exit(1);
    }
    let path = Path::new(&args[1]);
    let path = path.join("Route.BASELINE.zip");

    match parse_route_zip_file(path) {
        Ok(routes) => {
            if let Ok(df) = df!(
                "identifier" => routes.values().map(|route| route.identifier.clone()).collect::<Vec<_>>(),
                "prefix" => routes.values().map(|route| route.prefix.clone()).collect::<Vec<_>>(),
                "second_letter" => routes.values().map(|route| route.second_letter.clone()).collect::<Vec<_>>(),
                "number" => routes.values().map(|route| route.number.clone()).collect::<Vec<_>>(),
                "multiple_identifier" => routes.values().map(|route| route.multiple_identifier.clone()).collect::<Vec<_>>(),
            ) {
                println!("{df:?}");
            }
        }
        Err(e) => eprintln!("Error parsing route file: {e}"),
    }
}
