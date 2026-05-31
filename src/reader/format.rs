use anyhow::Result;
use std::fs::File;
use std::io::{BufRead, BufReader};

/// The detected format of a file — derived from extension + first-line sniff.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Jsonl,
    Csv,
    Raw,
}

impl FileFormat {
    /// Detect format by extension first, then confirm by sniffing the first line.
    /// A `.log` file whose first line is a JSON object is treated as JSONL.
    pub fn detect(path: &str) -> Self {
        let by_ext = if path.ends_with(".jsonl") || path.ends_with(".ndjson") {
            FileFormat::Jsonl
        } else if path.ends_with(".csv") || path.ends_with(".tsv") {
            FileFormat::Csv
        } else {
            FileFormat::Raw
        };

        if let Ok(first) = Self::read_first_line(path) {
            let trimmed = first.trim();
            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                return FileFormat::Jsonl;
            }
            if trimmed.starts_with('[') {
                return FileFormat::Raw; // JSON array, not JSONL
            }
        }

        by_ext
    }

    pub fn as_str(self) -> &'static str {
        match self {
            FileFormat::Jsonl => "jsonl",
            FileFormat::Csv => "csv",
            FileFormat::Raw => "raw",
        }
    }

    fn read_first_line(path: &str) -> Result<String> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        Ok(line)
    }
}
