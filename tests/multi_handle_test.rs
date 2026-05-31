/// Black-box tests for multiple file handles — concurrent open files,
/// close behavior, file_id independence.

use std::io::Write;
use tempfile::NamedTempFile;
use glance::reader::FileHandle;

fn file(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    let _ = std::fs::remove_file(format!("{}.glance_idx", f.path().to_str().unwrap()));
    f
}

fn open(f: &NamedTempFile) -> FileHandle {
    FileHandle::open(f.path().to_str().unwrap()).unwrap()
}

// ── Independent handles ───────────────────────────────────────────────────────

#[test]
fn two_handles_to_different_files_independent() {
    let f1 = file("file one content\n");
    let f2 = file("file two content\n");
    let h1 = open(&f1);
    let h2 = open(&f2);

    let lines1 = h1.read_lines(0, 1).unwrap();
    let lines2 = h2.read_lines(0, 1).unwrap();

    assert_eq!(lines1[0].content, "file one content");
    assert_eq!(lines2[0].content, "file two content");
}

#[test]
fn two_handles_to_same_file_both_read_correctly() {
    let f = file("shared line one\nshared line two\n");
    let h1 = open(&f);
    let h2 = open(&f);

    let l1 = h1.read_lines(0, 1).unwrap();
    let l2 = h2.read_lines(1, 1).unwrap();

    assert_eq!(l1[0].content, "shared line one");
    assert_eq!(l2[0].content, "shared line two");
}

#[test]
fn search_on_one_handle_does_not_affect_another() {
    let f1 = file("alpha\nbeta\n");
    let f2 = file("gamma\ndelta\n");
    let h1 = open(&f1);
    let h2 = open(&f2);

    let (r1, _) = h1.search("alpha", 10).unwrap();
    let (r2, _) = h2.search("gamma", 10).unwrap();

    assert_eq!(r1.len(), 1);
    assert_eq!(r2.len(), 1);
    assert_eq!(r1[0].content, "alpha");
    assert_eq!(r2[0].content, "gamma");
}

// ── Handle lifecycle ──────────────────────────────────────────────────────────

#[test]
fn handle_with_many_lines_total_lines_correct() {
    let content: String = (0..1000).map(|i| format!("line {}\n", i)).collect();
    let f = file(&content);
    let h = open(&f);
    assert_eq!(h.index.total_lines(), 1000);
}

#[test]
fn handle_reports_correct_file_size() {
    let content = "hello\n"; // 6 bytes
    let f = file(content);
    let h = open(&f);
    assert_eq!(h.index.file_size(), 6);
}

// ── Search across handles ─────────────────────────────────────────────────────

#[test]
fn count_on_different_handles_independent() {
    let f1 = file("error\nerror\nerror\n");
    let f2 = file("error\n");
    let h1 = open(&f1);
    let h2 = open(&f2);

    assert_eq!(h1.count("error").unwrap(), 3);
    assert_eq!(h2.count("error").unwrap(), 1);
}

// ── Path in handle ────────────────────────────────────────────────────────────

#[test]
fn handle_path_is_canonical() {
    let f = file("data\n");
    let path = f.path().to_str().unwrap();
    let h = open(&f);
    // Canonical path should be absolute
    assert!(h.path.starts_with('/'));
    // Should point to same file
    assert!(std::path::Path::new(&h.path).exists());
}
