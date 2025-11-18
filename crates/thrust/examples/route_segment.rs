use polars::prelude::*;
use std::env;
use std::path::Path;
use thrust::data::eurocontrol::aixm::route_segment::parse_route_segment_zip_file;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path_to_directory>", args[0]);
        std::process::exit(1);
    }
    let path = Path::new(&args[1]);
    let path = path.join("RouteSegment.BASELINE.zip");

    match parse_route_segment_zip_file(path) {
        Ok(route_segments) => {
            if let Ok(df) = df!(
                "identifier" => route_segments.values().map(|segment| segment.identifier.clone()).collect::<Vec<_>>(),
                "route_formed" => route_segments.values().map(|segment| segment.route_formed.clone()).collect::<Vec<_>>(),
                "start" => route_segments.values().map(|segment| segment.start.name()).collect::<Vec<_>>(),
                "end" => route_segments.values().map(|segment| segment.end.name()).collect::<Vec<_>>(),
            ) {
                println!("{df:?}");
            }
        }
        Err(e) => eprintln!("Error parsing route segment file: {e}"),
    }
}
