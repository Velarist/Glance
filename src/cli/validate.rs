use anyhow::Result;
use serde::Serialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use crate::security::validate_path;
use super::output::Format;

#[derive(Serialize)]
pub struct ValidateOutput {
    pub path: String,
    pub total_lines: u64,
    pub invalid_count: u64,
    pub invalid: Vec<InvalidLine>,
}

#[derive(Serialize)]
pub struct InvalidLine {
    pub line: u64,
    pub error: String,
    pub preview: String,
}

pub fn run(path: &str, fmt: Format) -> Result<()> {
    let path = validate_path(path)?;
    let file = File::open(&path)?;
    let reader = BufReader::with_capacity(64 * 1024, file);

    let mut total = 0u64;
    let mut invalid: Vec<InvalidLine> = Vec::new();

    for (idx, line) in reader.lines().enumerate() {
        let line = line?;
        total += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue; // blank lines are skipped, not invalid
        }
        if let Err(e) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let preview = if line.len() > 80 {
                format!("{}…", &line[..80])
            } else {
                line.clone()
            };
            invalid.push(InvalidLine {
                line: idx as u64 + 1,
                error: e.to_string(),
                preview,
            });
        }
    }

    let out = ValidateOutput {
        path: path.clone(),
        total_lines: total,
        invalid_count: invalid.len() as u64,
        invalid,
    };

    fmt.print(&out, |o| {
        if o.invalid.is_empty() {
            println!("✓ All {} lines are valid JSON", o.total_lines);
        } else {
            println!("✗ {} invalid line(s) out of {}:", o.invalid_count, o.total_lines);
            for inv in &o.invalid {
                println!("  Line {:>7}: {} — {}", inv.line, inv.error, inv.preview);
            }
        }
    });

    // Exit code 1 if any invalid lines found — useful for CI scripting
    if out.invalid_count > 0 {
        std::process::exit(1);
    }

    Ok(())
}
