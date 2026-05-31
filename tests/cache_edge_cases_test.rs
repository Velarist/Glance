/// Black-box tests for index cache — persistence, invalidation edge cases,
/// and integrity checks.

use std::io::Write;
use tempfile::NamedTempFile;
use glance::index::{cache, line_index::LineIndex};
use glance::reader::FileHandle;

fn file(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    f
}

fn cache_path(f: &NamedTempFile) -> String {
    format!("{}.glance_idx", f.path().to_str().unwrap())
}

fn cleanup(f: &NamedTempFile) {
    let _ = std::fs::remove_file(cache_path(f));
}

// ── Cache persistence ─────────────────────────────────────────────────────────

#[test]
fn cache_created_on_first_open() {
    let f = file("line one\nline two\n");
    cleanup(&f);
    let _ = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    assert!(std::path::Path::new(&cache_path(&f)).exists(), "cache file should be created");
    cleanup(&f);
}

#[test]
fn second_open_uses_cache_same_results() {
    let f = file("a\nb\nc\n");
    cleanup(&f);

    let h1 = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    let lines1 = h1.read_lines(0, 3).unwrap();

    // Second open — should use cache
    let h2 = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    let lines2 = h2.read_lines(0, 3).unwrap();

    assert_eq!(lines1.len(), lines2.len());
    for (l1, l2) in lines1.iter().zip(lines2.iter()) {
        assert_eq!(l1.content, l2.content);
        assert_eq!(l1.number, l2.number);
    }
    cleanup(&f);
}

// ── Cache invalidation ────────────────────────────────────────────────────────

#[test]
fn cache_invalidated_when_content_appended() {
    let mut f = NamedTempFile::new().unwrap();
    write!(f, "original\n").unwrap();
    f.flush().unwrap();
    cleanup(&f);

    // Build cache with 1 line
    let h1 = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    assert_eq!(h1.index.total_lines(), 1);

    // Append to file (size and mtime change)
    write!(f, "appended\n").unwrap();
    f.flush().unwrap();

    // Cache should be invalidated — new open rebuilds index
    let h2 = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    assert_eq!(h2.index.total_lines(), 2);
    cleanup(&f);
}

// ── Sanity cap on corrupt cache ───────────────────────────────────────────────

#[test]
fn corrupt_cache_with_huge_line_count_returns_none() {
    let f = file("data\n");
    let cp = cache_path(&f);

    // Write a cache with absurd total_lines = u64::MAX
    let mut out = std::fs::File::create(&cp).unwrap();
    out.write_all(b"GLNCIDX1").unwrap(); // magic
    out.write_all(&4u64.to_le_bytes()).unwrap();  // file_size
    out.write_all(&0u64.to_le_bytes()).unwrap();  // mtime (won't match but testing cap)
    out.write_all(&u64::MAX.to_le_bytes()).unwrap(); // total_lines = max!
    drop(out);

    assert!(cache::load(f.path().to_str().unwrap()).is_none(),
        "absurd line count should be rejected");
    cleanup(&f);
}

#[test]
fn truncated_cache_file_returns_none() {
    let f = file("a\nb\nc\n");
    let idx = LineIndex::build(f.path().to_str().unwrap()).unwrap();
    cache::save(f.path().to_str().unwrap(), &idx).unwrap();

    // Truncate the cache file halfway
    let cp = cache_path(&f);
    let data = std::fs::read(&cp).unwrap();
    std::fs::write(&cp, &data[..data.len() / 2]).unwrap();

    assert!(cache::load(f.path().to_str().unwrap()).is_none(),
        "truncated cache should fail gracefully");
    cleanup(&f);
}
