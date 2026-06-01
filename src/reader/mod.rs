pub mod csv;
pub mod format;
pub mod pretty;
pub mod search;
pub mod stream;

use anyhow::Result;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};

use crate::index::{cache, line_index::LineIndex};
use crate::protocol::response::Line;

pub use format::FileFormat;
pub use stream::{read_lines_direct, stream_count, stream_count_regex, stream_search, stream_search_regex};

/// An open file with a pre-built line index for O(1) random-access reads.
pub struct FileHandle {
    pub path: String,
    pub format: FileFormat,
    pub index: LineIndex,
}

impl FileHandle {
    /// Open a file: detect format, load or build the line index.
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
        Ok(Self { path: path.to_string(), format, index })
    }

    /// Read `limit` lines starting from `offset` (0-indexed).
    /// Uses the line index to seek directly — no scan from the beginning.
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
            if n == 0 { break; }
            let content = buf.trim_end_matches('\n').trim_end_matches('\r').to_string();
            let fields = if matches!(self.format, FileFormat::Csv) {
                Some(csv::parse_line(&content, csv::delimiter_for(&self.path)))
            } else {
                None
            };
            lines.push(Line { number: line_num, content, fields });
        }

        Ok(lines)
    }

    // ── Delegation to search module ────────────────────────────────────────────
    // These methods delegate to standalone functions so callers don't need to
    // know which sub-module handles each operation.

    pub fn search(&self, query: &str, max_results: usize) -> Result<(Vec<crate::protocol::response::SearchResult>, bool)> {
        search::search(&self.path, query, max_results)
    }

    pub fn search_regex(&self, pattern: &str, max_results: usize) -> Result<(Vec<crate::protocol::response::SearchResult>, bool)> {
        search::search_regex(&self.path, pattern, max_results)
    }

    /// Search with context lines before/after each match.
    /// Uses the line index for O(1) seeks when fetching context.
    pub fn search_with_context(
        &self,
        query: &str,
        max_results: usize,
        use_regex: bool,
        before: usize,
        after: usize,
    ) -> Result<(Vec<crate::protocol::response::SearchResult>, bool)> {
        let (mut results, truncated) = if use_regex {
            search::search_regex(&self.path, query, max_results)?
        } else {
            search::search(&self.path, query, max_results)?
        };

        if before > 0 || after > 0 {
            for r in &mut results {
                let (ctx_before, ctx_after) =
                    search::fetch_context(&self.path, &self.index, r.line_number, before, after)?;
                r.context_before = ctx_before;
                r.context_after = ctx_after;
            }
        }

        Ok((results, truncated))
    }

    pub fn count(&self, query: &str) -> Result<u64> {
        search::count(&self.path, query)
    }

    pub fn count_regex(&self, pattern: &str) -> Result<u64> {
        search::count_regex(&self.path, pattern)
    }
}
