/// Black-box tests for read operations — edge cases, boundary conditions,
/// format interactions. Written without looking at implementation details.

use std::io::Write;
use tempfile::NamedTempFile;
use glance::reader::FileHandle;

fn file(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    // Remove any stale cache
    let _ = std::fs::remove_file(format!("{}.glance_idx", f.path().to_str().unwrap()));
    f
}

fn open(f: &NamedTempFile) -> FileHandle {
    FileHandle::open(f.path().to_str().unwrap()).unwrap()
}

// ── Boundary reads ────────────────────────────────────────────────────────────

#[test]
fn read_last_valid_offset() {
    let f = file("a\nb\nc\n");
    let h = open(&f);
    // offset 2 = third line (0-indexed), valid
    let lines = h.read_lines(2, 1).unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].content, "c");
}

#[test]
fn read_exactly_one_past_end_returns_error() {
    let f = file("a\nb\nc\n");
    let h = open(&f);
    // 3 lines → offset 3 is out of range
    assert!(h.read_lines(3, 1).is_err());
}

#[test]
fn read_limit_larger_than_remaining_returns_rest() {
    let f = file("x\ny\nz\n");
    let h = open(&f);
    // Offset 1 + limit 9999 → should return only 2 remaining lines
    let lines = h.read_lines(1, 9999).unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].content, "y");
    assert_eq!(lines[1].content, "z");
}

#[test]
fn read_single_line_file_no_newline() {
    let f = file("only line");
    let h = open(&f);
    assert_eq!(h.index.total_lines(), 1);
    let lines = h.read_lines(0, 1).unwrap();
    assert_eq!(lines[0].content, "only line");
}

#[test]
fn read_preserves_line_numbers() {
    let f = file("first\nsecond\nthird\n");
    let h = open(&f);
    let lines = h.read_lines(1, 2).unwrap();
    assert_eq!(lines[0].number, 1); // 0-indexed, second line
    assert_eq!(lines[1].number, 2);
}

#[test]
fn read_very_long_line() {
    let long = "x".repeat(100_000);
    let content = format!("before\n{}\nafter\n", long);
    let f = file(&content);
    let h = open(&f);
    assert_eq!(h.index.total_lines(), 3);
    let lines = h.read_lines(1, 1).unwrap();
    assert_eq!(lines[0].content.len(), 100_000);
}

#[test]
fn read_file_with_only_newlines() {
    let f = file("\n\n\n");
    let h = open(&f);
    assert_eq!(h.index.total_lines(), 3);
    let lines = h.read_lines(0, 3).unwrap();
    assert!(lines.iter().all(|l| l.content.is_empty()));
}

// ── Pretty-print mode ─────────────────────────────────────────────────────────

#[test]
fn pretty_mode_expands_valid_json() {
    let f = NamedTempFile::with_suffix(".jsonl").unwrap();
    {
        let mut w = std::io::BufWriter::new(f.as_file());
        writeln!(w, r#"{{"a":1,"b":2}}"#).unwrap();
    }
    let _ = std::fs::remove_file(format!("{}.glance_idx", f.path().to_str().unwrap()));
    let _h = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    // Use read_lines_direct with pretty=true via the public helper
    let result = glance::reader::read_lines_direct(
        f.path().to_str().unwrap(), 0, 0, 1, true, glance::reader::FileFormat::Jsonl
    ).unwrap();
    assert!(result[0].content.contains('\n'), "pretty-printed JSON should have newlines");
    assert!(result[0].content.contains("\"a\""));
}

#[test]
fn pretty_mode_fallback_on_invalid_json() {
    let f = NamedTempFile::with_suffix(".jsonl").unwrap();
    {
        let mut w = std::io::BufWriter::new(f.as_file());
        // Use write! with a raw string to avoid format escaping confusion
        w.write_all(b"not valid json <<<\n").unwrap();
    }
    let _ = std::fs::remove_file(format!("{}.glance_idx", f.path().to_str().unwrap()));
    let _h = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    let result = glance::reader::read_lines_direct(
        f.path().to_str().unwrap(), 0, 0, 1, true, glance::reader::FileFormat::Jsonl
    ).unwrap();
    // Should fall back to raw content, not panic
    assert_eq!(result[0].content, "not valid json <<<");
}
