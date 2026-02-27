use std::env;

use thrust::data::faa::nasr::parse_field15_data_from_nasr_zip;
use thrust::data::faa::nat::{fetch_nat_bulletin, resolve_named_points_with_nasr};

fn main() {
    let args: Vec<String> = env::args().collect();
    let nasr_zip = args.get(1).map(|s| s.as_str());

    match fetch_nat_bulletin() {
        Ok(mut bulletin) => {
            if let Some(nasr_zip) = nasr_zip {
                match parse_field15_data_from_nasr_zip(nasr_zip) {
                    Ok(data) => {
                        let resolved = resolve_named_points_with_nasr(&mut bulletin, &data.points);
                        println!("Resolved NAT named points with NASR: {resolved}");
                    }
                    Err(e) => eprintln!("Could not load NASR points for resolution: {e}"),
                }
            }

            println!("FAA NAT bulletin");
            println!("- updated_at: {:?}", bulletin.updated_at);
            println!("- tmi: {:?}", bulletin.tmi);
            println!("- tracks: {}", bulletin.tracks.len());

            for track in bulletin.tracks.iter().take(10) {
                let route = track
                    .route_points
                    .iter()
                    .map(|p| {
                        if let (Some(lat), Some(lon)) = (p.latitude, p.longitude) {
                            format!("{}({lat:.2},{lon:.2})", p.token)
                        } else {
                            p.token.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                println!(
                    "  - {} dir={:?} route={} east={:?} west={:?}",
                    track.track_id,
                    track.direction(),
                    route,
                    track.east_levels,
                    track.west_levels,
                );
            }
            if bulletin.tracks.len() > 10 {
                println!("  ... ({} more tracks)", bulletin.tracks.len() - 10);
            }
        }
        Err(e) => eprintln!("Error fetching/parsing FAA NAT bulletin: {e}"),
    }
}
