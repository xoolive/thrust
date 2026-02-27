use thrust::data::faa::arcgis::parse_all_faa_open_data;

fn main() {
    match parse_all_faa_open_data() {
        Ok(data) => {
            println!("FAA open datasets loaded:");
            println!("- ATS routes: {}", data.ats_routes.len());
            println!("- Designated points: {}", data.designated_points.len());
            println!("- Navaid components: {}", data.navaid_components.len());
            println!("- Airspace boundary: {}", data.airspace_boundary.len());
            println!("- Class airspace: {}", data.class_airspace.len());
            println!("- Special use airspace: {}", data.special_use_airspace.len());
            println!("- Route airspace: {}", data.route_airspace.len());
            println!("- Prohibited airspace: {}", data.prohibited_airspace.len());
        }
        Err(e) => eprintln!("Error loading FAA open datasets: {e}"),
    }
}
