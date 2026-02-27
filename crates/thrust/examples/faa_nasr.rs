use std::{env, path::PathBuf};
use thrust::data::faa::nasr::{load_nasr_cycle_summary, parse_field15_data_from_nasr_zip};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || args.len() > 3 {
        eprintln!("Usage: {} <airac_code_YYCC> [output_dir]", args[0]);
        std::process::exit(1);
    }

    let airac_code = &args[1];
    let output_dir = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/faa_nasr"));

    match load_nasr_cycle_summary(airac_code, &output_dir) {
        Ok(summary) => {
            println!("FAA NASR cycle {}", summary.airac_code);
            println!("- effective date: {}", summary.effective_date);
            println!("- zip path: {}", summary.zip_path);
            println!("- files: {}", summary.files.len());

            for file in summary.files.iter().take(20) {
                println!(
                    "  - {} | size={} | lines={:?} | cols={:?} | delim={:?}",
                    file.name, file.size_bytes, file.line_count, file.header_columns, file.delimiter
                );
            }
            if summary.files.len() > 20 {
                println!("  ... ({} more files)", summary.files.len() - 20);
            }

            if let Ok(field15_data) = parse_field15_data_from_nasr_zip(&summary.zip_path) {
                println!("NASR Field15-oriented entities:");
                println!("- points: {}", field15_data.points.len());
                println!("- airway segments: {}", field15_data.airways.len());
                println!("- SID designators: {}", field15_data.sid_designators.len());
                println!("- STAR designators: {}", field15_data.star_designators.len());
                println!("- SID legs: {}", field15_data.sid_legs.len());
                println!("- STAR legs: {}", field15_data.star_legs.len());
            }
        }
        Err(e) => eprintln!("Error loading FAA NASR cycle: {e}"),
    }
}
