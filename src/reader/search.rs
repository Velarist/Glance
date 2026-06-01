use anyhow::Result;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};

use crate::protocol::response::SearchResult;
pub use crate::protocol::response::ContextLine;

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
            let char_start = line_lower[..byte_pos].chars().count();
            let char_end = char_start + line_lower[byte_pos..byte_pos + query_lower.len()].chars().count();
            results.push(SearchResult {
                line_number: line_num as u64,
                content: line,
                match_start: char_start,
                match_end: char_end,
                context_before: vec![],
                context_after: vec![],
            });
            if results.len() >= max_results {
                return Ok((results, true));
            }
        }
    }

    Ok((results, false))
}

/// Regex search. Compiles the pattern once, streams the file.
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
                context_before: vec![],
                context_after: vec![],
            });
            if results.len() >= max_results {
                return Ok((results, true));
            }
        }
    }

    Ok((results, false))
}

/// Fetch context lines around a match using a pre-built line index.
/// Reads `before` lines before and `after` lines after `match_line`.
pub fn fetch_context(
    path: &str,
    index: &crate::index::line_index::LineIndex,
    match_line: u64,
    before: usize,
    after: usize,
) -> Result<(Vec<ContextLine>, Vec<ContextLine>)> {
    use crate::reader::stream::read_lines_direct;
    use crate::reader::format::FileFormat;

    let total = index.total_lines();

    // context_before
    let before_start = match_line.saturating_sub(before as u64);
    let ctx_before = if before > 0 && before_start < match_line {
        let byte_off = index.line_offset(before_start).unwrap_or(0);
        let lines = read_lines_direct(path, byte_off, before_start, match_line, false, FileFormat::Raw)?;
        lines.into_iter().map(|l| ContextLine { line_number: l.number, content: l.content }).collect()
    } else {
        vec![]
    };

    // context_after
    let after_end = (match_line + 1 + after as u64).min(total);
    let ctx_after = if after > 0 && match_line + 1 < total {
        let byte_off = index.line_offset(match_line + 1).unwrap_or(0);
        let lines = read_lines_direct(path, byte_off, match_line + 1, after_end, false, FileFormat::Raw)?;
        lines.into_iter().map(|l| ContextLine { line_number: l.number, content: l.content }).collect()
    } else {
        vec![]
    };

    Ok((ctx_before, ctx_after))
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
