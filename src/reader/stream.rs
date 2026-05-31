/// Standalone streaming operations — no index required.
/// Used by the RPC server after releasing the file store lock,
/// to avoid holding the lock during long disk I/O.

use anyhow::Result;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};

use crate::protocol::response::{Line, SearchResult};
use super::format::FileFormat;
use super::{csv, pretty, search as search_ops};

/// Read lines from a pre-computed byte offset without holding any lock.
/// Handles pretty-printing and CSV field parsing based on `format`.
pub fn read_lines_direct(
    path: &str,
    byte_offset: u64,
    start: u64,
    end: u64,
    pretty_mode: bool,
    format: FileFormat,
) -> Result<Vec<Line>> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(byte_offset))?;
    let mut reader = BufReader::new(file);
    let mut lines = Vec::with_capacity((end - start) as usize);
    let mut buf = String::new();

    for line_num in start..end {
        buf.clear();
        let n = reader.read_line(&mut buf)?;
        if n == 0 { break; }
        let raw = buf.trim_end_matches('\n').trim_end_matches('\r').to_string();
        let content = if pretty_mode && format == FileFormat::Jsonl {
            pretty::json(&raw)
        } else {
            raw
        };
        let fields = if format == FileFormat::Csv {
            Some(csv::parse_line(&content, csv::delimiter_for(path)))
        } else {
            None
        };
        lines.push(Line { number: line_num, content, fields });
    }

    Ok(lines)
}

/// Standalone substring search — streams `path` without an index.
pub fn stream_search(path: &str, query: &str, max_results: usize) -> Result<(Vec<SearchResult>, bool)> {
    search_ops::search(path, query, max_results)
}

/// Standalone regex search.
pub fn stream_search_regex(path: &str, pattern: &str, max_results: usize) -> Result<(Vec<SearchResult>, bool)> {
    search_ops::search_regex(path, pattern, max_results)
}

/// Standalone count.
pub fn stream_count(path: &str, query: &str) -> Result<u64> {
    search_ops::count(path, query)
}

/// Standalone regex count.
pub fn stream_count_regex(path: &str, pattern: &str) -> Result<u64> {
    search_ops::count_regex(path, pattern)
}
