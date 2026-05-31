use anyhow::Result;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};

use super::line_index::LineIndex;

// Magic bytes guard against loading a corrupt or unrelated file as an index.
const MAGIC: &[u8; 8] = b"GLNCIDX1";

/// Return the path where the index cache for `source_path` would be stored.
fn cache_path(source_path: &str) -> String {
    format!("{}.glance_idx", source_path)
}

/// Load a cached index if it exists and is still valid for `source_path`.
/// Validity: magic matches, cached file_size matches current file size, mtime matches.
pub fn load(source_path: &str) -> Option<LineIndex> {
    let cp = cache_path(source_path);
    let meta = fs::metadata(source_path).ok()?;
    let current_size = meta.len();
    let current_mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    let file = File::open(&cp).ok()?;
    let mut reader = BufReader::new(file);

    let mut magic = [0u8; 8];
    reader.read_exact(&mut magic).ok()?;
    if &magic != MAGIC {
        return None;
    }

    let cached_size = read_u64(&mut reader)?;
    let cached_mtime = read_u64(&mut reader)?;
    let total_lines = read_u64(&mut reader)?;

    if cached_size != current_size || cached_mtime != current_mtime {
        return None;
    }

    // Sanity cap: reject corrupt cache claiming absurd line counts.
    // 500M lines * 8 bytes = 4GB index — already extreme for any real file.
    const MAX_LINES: u64 = 500_000_000;
    if total_lines > MAX_LINES {
        tracing::warn!(total_lines, "cache rejected: line count exceeds sanity limit");
        return None;
    }

    let mut offsets = Vec::with_capacity(total_lines as usize);
    for _ in 0..total_lines {
        offsets.push(read_u64(&mut reader)?);
    }

    tracing::debug!(source = source_path, total_lines, "loaded index from cache");
    Some(LineIndex::from_parts(offsets, current_size))
}

/// Save `index` to disk so it can be reloaded on next daemon start.
pub fn save(source_path: &str, index: &LineIndex) -> Result<()> {
    let cp = cache_path(source_path);
    let meta = fs::metadata(source_path)?;
    let file_size = meta.len();
    let mtime = meta
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let file = File::create(&cp)?;
    let mut writer = BufWriter::new(file);

    writer.write_all(MAGIC)?;
    write_u64(&mut writer, file_size)?;
    write_u64(&mut writer, mtime)?;
    write_u64(&mut writer, index.total_lines())?;
    for &offset in index.offsets() {
        write_u64(&mut writer, offset)?;
    }
    writer.flush()?;

    tracing::debug!(cache = cp, total_lines = index.total_lines(), "index cache saved");
    Ok(())
}

fn read_u64(r: &mut impl Read) -> Option<u64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf).ok()?;
    Some(u64::from_le_bytes(buf))
}

fn write_u64(w: &mut impl Write, v: u64) -> Result<()> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}
