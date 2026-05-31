pub mod csv;

use anyhow::Result;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};

use crate::index::{cache, line_index::LineIndex};
use crate::protocol::response::{Line, SearchResult};

pub enum FileFormat {
    Jsonl,
    Csv,
    Raw,
}

impl FileFormat {
    /// Detect format by extension first, then confirm by sniffing the first line.
    /// This catches cases like a `.log` file that actually contains JSONL.
    pub fn detect(path: &str) -> Self {
        let by_ext = if path.ends_with(".jsonl") || path.ends_with(".ndjson") {
            FileFormat::Jsonl
        } else if path.ends_with(".csv") || path.ends_with(".tsv") {
            FileFormat::Csv
        } else {
            FileFormat::Raw
        };

        // Sniff first line to confirm or override extension-based guess
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

    fn read_first_line(path: &str) -> Result<String> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        Ok(line)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            FileFormat::Jsonl => "jsonl",
            FileFormat::Csv => "csv",
            FileFormat::Raw => "raw",
        }
    }
}

pub struct FileHandle {
    pub path: String,
    pub format: FileFormat,
    pub index: LineIndex,
}

impl FileHandle {
    pub fn open(path: &str) -> Result<Self> {
        let format = FileFormat::detect(path);
        let index = match cache::load(path) {
            Some(cached) => cached,
            None => {
                let idx = LineIndex::build(path)?;
                if let Err(e) = cache::save(path, &idx) {
                    tracing::warn!(error = %e, "failed to save index cache");
                }
                idx
            }
        };
        Ok(Self {
            path: path.to_string(),
            format,
            index,
        })
    }

    /// Read `limit` lines starting from `offset` (0-indexed line number).
    /// Uses the line index to seek directly — no need to scan from the beginning.
    pub fn read_lines(&self, offset: u64, limit: u64) -> Result<Vec<Line>> {
        let total = self.index.total_lines();

        if offset >= total {
            anyhow::bail!("offset {offset} out of range (file has {total} lines)");
        }

        let start = offset;
        let end = (start + limit).min(total);

        if start >= end {
            return Ok(vec![]);
        }

        let byte_offset = match self.index.line_offset(start) {
            Some(o) => o,
            None => return Ok(vec![]),
        };

        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(byte_offset))?;
        let mut reader = BufReader::new(file);

        let mut lines = Vec::with_capacity((end - start) as usize);
        let mut buf = String::new();

        for line_num in start..end {
            buf.clear();
            let n = reader.read_line(&mut buf)?;
            if n == 0 {
                break;
            }
            let content = buf.trim_end_matches('\n').trim_end_matches('\r').to_string();
            let fields = if matches!(self.format, FileFormat::Csv) {
                Some(csv::parse_line(&content, csv::delimiter_for(&self.path)))
            } else {
                None
            };
            lines.push(Line {
                number: line_num,
                content,
                fields,
            });
        }

        Ok(lines)
    }

    /// Case-insensitive substring search. Streams the file line-by-line — never loads into RAM.
    /// Returns (results, truncated). `truncated` is true when the stream was cut short at max_results.
    pub fn search(&self, query: &str, max_results: usize) -> Result<(Vec<SearchResult>, bool)> {
        let file = File::open(&self.path)?;
        let reader = BufReader::with_capacity(64 * 1024, file);
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            let line_lower = line.to_lowercase();
            if let Some(byte_pos) = line_lower.find(&query_lower) {
                // Convert byte offsets → char offsets so JavaScript slice() works correctly
                // for non-ASCII content (é, CJK, etc.)
                let char_start = line[..byte_pos].chars().count();
                let char_end = char_start + line[byte_pos..byte_pos + query.len()].chars().count();
                results.push(SearchResult {
                    line_number: line_num as u64,
                    content: line,
                    match_start: char_start,
                    match_end: char_end,
                });
                if results.len() >= max_results {
                    return Ok((results, true));
                }
            }
        }

        Ok((results, false))
    }

    /// Regex search. Compiles the pattern once, then streams the file.
    /// Returns an error immediately if the pattern is invalid — no partial results.
    pub fn search_regex(&self, pattern: &str, max_results: usize) -> Result<(Vec<SearchResult>, bool)> {
        let re = Regex::new(pattern)
            .map_err(|e| anyhow::anyhow!("invalid regex: {e}"))?;

        let file = File::open(&self.path)?;
        let reader = BufReader::with_capacity(64 * 1024, file);
        let mut results = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            if let Some(m) = re.find(&line) {
                let char_start = line[..m.start()].chars().count();
                let char_end = char_start + line[m.start()..m.end()].chars().count();
                results.push(SearchResult {
                    line_number: line_num as u64,
                    content: line,
                    match_start: char_start,
                    match_end: char_end,
                });
                if results.len() >= max_results {
                    return Ok((results, true));
                }
            }
        }

        Ok((results, false))
    }

    /// Count matching lines without accumulating results — O(1) memory regardless of file size.
    pub fn count(&self, query: &str) -> Result<u64> {
        let file = File::open(&self.path)?;
        let reader = BufReader::with_capacity(64 * 1024, file);
        let query_lower = query.to_lowercase();
        let mut count = 0u64;
        for line in reader.lines() {
            if line?.to_lowercase().contains(&query_lower) {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Regex variant of `count`.
    pub fn count_regex(&self, pattern: &str) -> Result<u64> {
        let re = Regex::new(pattern)
            .map_err(|e| anyhow::anyhow!("invalid regex: {e}"))?;
        let file = File::open(&self.path)?;
        let reader = BufReader::with_capacity(64 * 1024, file);
        let mut count = 0u64;
        for line in reader.lines() {
            if re.is_match(&line?) {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Pretty-print a single JSONL line. Falls back to raw content on parse failure.
    fn pretty_json(raw: &str) -> String {
        match serde_json::from_str::<serde_json::Value>(raw) {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| raw.to_string()),
            Err(_) => raw.to_string(),
        }
    }

    /// Read lines with optional JSONL pretty-printing applied.
    pub fn read_lines_pretty(&self, offset: u64, limit: u64) -> Result<Vec<Line>> {
        let mut lines = self.read_lines(offset, limit)?;
        if matches!(self.format, FileFormat::Jsonl) {
            for line in &mut lines {
                line.content = Self::pretty_json(&line.content);
            }
        }
        Ok(lines)
    }

}
