use std::{env, path::Path};

use thrust::data::eurocontrol::ddr::airspaces::{
    find_file_with_prefix_suffix, parse_are_file, parse_sls_file, parse_spc_file,
};
use thrust::data::eurocontrol::ddr::freeroute::parse_freeroute_dir;
use thrust::data::eurocontrol::ddr::navpoints::parse_navpoints_dir;
use thrust::data::eurocontrol::ddr::procedures::parse_sid_star_dir;
use thrust::data::eurocontrol::ddr::routes::parse_routes_dir;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path_to_nm_ddr_directory>", args[0]);
        std::process::exit(1);
    }

    let path = Path::new(&args[1]);

    match parse_navpoints_dir(path) {
        Ok(navpoints) => {
            println!("DDR decode from {}", path.display());
            println!("- navpoints: {}", navpoints.len());

            match parse_routes_dir(path) {
                Ok(routes) => println!("- route points: {}", routes.len()),
                Err(e) => eprintln!("- routes: error {e}"),
            }

            let sectors_are = find_file_with_prefix_suffix(path, "Sectors_", ".are");
            let sectors_sls = find_file_with_prefix_suffix(path, "Sectors_", ".sls");
            let sectors_spc = find_file_with_prefix_suffix(path, "Sectors_", ".spc");
            if let (Some(are), Some(sls), Some(spc)) = (sectors_are, sectors_sls, sectors_spc) {
                match parse_are_file(&are).and_then(|polys| parse_sls_file(&sls, &polys).map(|layers| (polys, layers)))
                {
                    Ok((polys, layers)) => {
                        println!("- sector polygons: {}", polys.len());
                        println!("- sector layers: {}", layers.len());
                    }
                    Err(e) => eprintln!("- sectors: error {e}"),
                }

                match parse_spc_file(spc) {
                    Ok(spc) => println!("- collapsed sector relations: {}", spc.len()),
                    Err(e) => eprintln!("- spc: error {e}"),
                }
            } else {
                eprintln!("- sectors: files not found");
            }

            match parse_freeroute_dir(path, &navpoints) {
                Ok(fra) => {
                    println!("- free-route areas: {}", fra.areas.len());
                    println!("- free-route points: {}", fra.points.len());
                    let with_coords = fra
                        .points
                        .iter()
                        .filter(|p| p.latitude.is_some() && p.longitude.is_some())
                        .count();
                    println!("- free-route points with coordinates: {}", with_coords);
                }
                Err(e) => eprintln!("- free-route: error {e}"),
            }

            match parse_sid_star_dir(path) {
                Ok((sids, stars)) => {
                    println!("- ddr SID refs: {}", sids.len());
                    println!("- ddr STAR refs: {}", stars.len());
                }
                Err(e) => eprintln!("- ddr sid/star: error {e}"),
            }
        }
        Err(e) => eprintln!("Error decoding DDR input: {e}"),
    }
}
