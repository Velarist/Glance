use std::io::Write;
use tempfile::NamedTempFile;
use glance::index::{cache, line_index::LineIndex};

fn tmp(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    f
}

#[test]
fn save_and_load_produces_identical_index() {
    let f = tmp("alpha\nbeta\ngamma\n");
    let path = f.path().to_str().unwrap();

    let original = LineIndex::build(path).unwrap();
    cache::save(path, &original).unwrap();

    let loaded = cache::load(path).expect("cache should load successfully");
    assert_eq!(original.total_lines(), loaded.total_lines());
    assert_eq!(original.file_size(), loaded.file_size());
    for i in 0..original.total_lines() {
        assert_eq!(original.line_offset(i), loaded.line_offset(i));
    }

    // clean up cache file
    let _ = std::fs::remove_file(format!("{}.glance_idx", path));
}

#[test]
fn load_returns_none_when_no_cache() {
    let f = tmp("some content\n");
    let path = f.path().to_str().unwrap();
    // No cache file written — should return None
    assert!(cache::load(path).is_none());
}

#[test]
fn load_returns_none_after_file_grows() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"line one\n").unwrap();
    f.flush().unwrap();
    let path = f.path().to_str().unwrap();

    let idx = LineIndex::build(path).unwrap();
    cache::save(path, &idx).unwrap();

    // Append to the file — size changes, cache should be invalidated
    let mut file = std::fs::OpenOptions::new().append(true).open(path).unwrap();
    file.write_all(b"line two\n").unwrap();
    drop(file);

    assert!(cache::load(path).is_none(), "cache should be invalidated after file grows");

    let _ = std::fs::remove_file(format!("{}.glance_idx", path));
}

#[test]
fn corrupt_cache_magic_returns_none() {
    let f = tmp("data\n");
    let path = f.path().to_str().unwrap();
    let cache_path = format!("{}.glance_idx", path);

    // Write garbage as the cache file
    std::fs::write(&cache_path, b"BADMAGIC garbage data here!!!").unwrap();

    assert!(cache::load(path).is_none(), "corrupt magic should return None");

    let _ = std::fs::remove_file(&cache_path);
}
