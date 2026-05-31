use anyhow::Result;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};

use crate::protocol::response::SearchResult;

/// Case-insensitive substring search. Streams the file line-by-line.
/// Returns (results, truncated). `truncated` is true when cut short at max_results.
pub fn search(path: &str, query: &str, max_results: usize) -> Result<(Vec<SearchResult>, bool)> {
    if query.is_empty() {
        anyhow::bail!("query must not be empty");
    }
    let file = File::open(path)?;
    let reader = BufReader::with_capacity(64 * 1024, file);
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        let line_lower = line.to_lowercase();
        if let Some(byte_pos) = line_lower.find(&query_lower) {
            // Use line_lower for slicing — lowercasing can change byte lengths for some
            // Unicode chars (e.g. ẞ → ß), so byte_pos is only valid in line_lower.
            let char_start = line_lower[..byte_pos].chars().count();
            let char_end = char_start + line_lower[byte_pos..byte_pos + query_lower.len()].chars().count();
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

/// Regex search. Compiles the pattern once, streams the file.
/// Returns an error immediately if the pattern is invalid.
pub fn search_regex(path: &str, pattern: &str, max_results: usize) -> Result<(Vec<SearchResult>, bool)> {
    let re = Regex::new(pattern)
        .map_err(|e| anyhow::anyhow!("invalid regex: {e}"))?;

    let file = File::open(path)?;
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

/// Count matching lines — O(1) memory regardless of file size.
pub fn count(path: &str, query: &str) -> Result<u64> {
    if query.is_empty() {
        anyhow::bail!("query must not be empty");
    }
    let file = File::open(path)?;
    let reader = BufReader::with_capacity(64 * 1024, file);
    let query_lower = query.to_lowercase();
    let mut n = 0u64;
    for line in reader.lines() {
        if line?.to_lowercase().contains(&query_lower) {
            n += 1;
        }
    }
    Ok(n)
}

/// Regex variant of count.
pub fn count_regex(path: &str, pattern: &str) -> Result<u64> {
    let re = Regex::new(pattern)
        .map_err(|e| anyhow::anyhow!("invalid regex: {e}"))?;
    let file = File::open(path)?;
    let reader = BufReader::with_capacity(64 * 1024, file);
    let mut n = 0u64;
    for line in reader.lines() {
        if re.is_match(&line?) {
            n += 1;
        }
    }
    Ok(n)
}
