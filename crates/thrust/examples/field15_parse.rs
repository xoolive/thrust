use std::io::{self, BufRead};
use thrust::data::field15::{Field15Element, Field15Parser};

fn main() {
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
        let elements: Vec<Field15Element> = Field15Parser::parse(&line);
        match serde_json::to_string(&elements) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("JSON serialization error: {}", e),
        }
    }
}
