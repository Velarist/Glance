use anyhow::Result;
use std::fs::File;
use std::io::{BufRead, BufReader};

/// Stores the byte offset of every line start so we can seek directly to any line in O(1).
/// Building the index requires one full pass over the file but is done once per open.
pub struct LineIndex {
    offsets: Vec<u64>,
    file_size: u64,
}

impl LineIndex {
    pub fn build(path: &str) -> Result<Self> {
        let file = File::open(path)?;
        let file_size = file.metadata()?.len();
        let mut reader = BufReader::with_capacity(64 * 1024, file);

        // Only push the initial offset if the file is non-empty.
        // An empty file has zero lines, not one.
        let mut offsets = if file_size > 0 { vec![0u64] } else { vec![] };
        let mut pos: u64 = 0;
        let mut buf = Vec::new();

        loop {
            buf.clear();
            let n = reader.read_until(b'\n', &mut buf)?;
            if n == 0 {
                break;
            }
            pos += n as u64;
            if pos < file_size {
                offsets.push(pos);
            }
        }

        Ok(Self { offsets, file_size })
    }

    pub fn total_lines(&self) -> u64 {
        self.offsets.len() as u64
    }

    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Returns the byte offset of the start of `line_number` (0-indexed).
    pub fn line_offset(&self, line_number: u64) -> Option<u64> {
        self.offsets.get(line_number as usize).copied()
    }

    /// Reconstruct an index from a pre-built offsets vec (used by the cache loader).
    pub fn from_parts(offsets: Vec<u64>, file_size: u64) -> Self {
        Self { offsets, file_size }
    }

    /// Read-only view of offsets (used when saving cache to disk).
    pub fn offsets(&self) -> &[u64] {
        &self.offsets
    }
}
