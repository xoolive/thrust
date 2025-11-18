use std::env;
use std::io;
use std::io::BufRead;
use std::path::Path;
use thrust::data::eurocontrol::database::AirwayDatabase;
use thrust::data::field15::Field15Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path_to_aixm_folder>", args[0]);
        std::process::exit(1);
    }

    let aixm_path = Path::new(&args[1]);
    if !aixm_path.exists() {
        eprintln!("Error: Path does not exist: {}", aixm_path.display());
        std::process::exit(1);
    }

    eprintln!("Loading AIXM data from: {}", aixm_path.display());
    let db = AirwayDatabase::new(aixm_path)?;

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => {
                let trimmed = l.trim();
                let trimmed = trimmed
                    .strip_prefix('"')
                    .or_else(|| trimmed.strip_prefix('\''))
                    .unwrap_or(trimmed);
                let trimmed = trimmed
                    .strip_suffix('"')
                    .or_else(|| trimmed.strip_suffix('\''))
                    .unwrap_or(trimmed);
                trimmed.to_string()
            }
            Err(_) => continue,
        };
        if line.is_empty() {
            continue;
        }

        let elements = Field15Parser::parse(&line);
        let enriched = db.enrich_route(elements);

        match serde_json::to_string(&enriched) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("JSON serialization error: {}", e),
        }
    }

    Ok(())
}
