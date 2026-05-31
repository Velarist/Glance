/// Black-box tests for format detection — extension-based and sniff-based.
/// Tests that the right format is detected for various file types and contents.

use std::io::Write;
use tempfile::{Builder, NamedTempFile};
use glance::reader::FileHandle;

fn file_with_ext(ext: &str, content: &str) -> NamedTempFile {
    let mut f = Builder::new().suffix(ext).tempfile().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    let _ = std::fs::remove_file(format!("{}.glance_idx", f.path().to_str().unwrap()));
    f
}

// ── Extension-based detection ─────────────────────────────────────────────────

#[test]
fn jsonl_extension_detected_as_jsonl() {
    let f = file_with_ext(".jsonl", "{\"a\":1}\n");
    let h = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    assert_eq!(h.format.as_str(), "jsonl");
}

#[test]
fn ndjson_extension_detected_as_jsonl() {
    let f = file_with_ext(".ndjson", "{\"a\":1}\n");
    let h = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    assert_eq!(h.format.as_str(), "jsonl");
}

#[test]
fn csv_extension_detected_as_csv() {
    let f = file_with_ext(".csv", "a,b,c\n1,2,3\n");
    let h = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    assert_eq!(h.format.as_str(), "csv");
}

#[test]
fn tsv_extension_detected_as_csv() {
    let f = file_with_ext(".tsv", "a\tb\tc\n1\t2\t3\n");
    let h = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    assert_eq!(h.format.as_str(), "csv");
}

#[test]
fn log_extension_defaults_to_raw() {
    let f = file_with_ext(".log", "2024-01-01 plain log line\n");
    let h = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    assert_eq!(h.format.as_str(), "raw");
}

#[test]
fn txt_extension_defaults_to_raw() {
    let f = file_with_ext(".txt", "just text\n");
    let h = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    assert_eq!(h.format.as_str(), "raw");
}

// ── Sniff-based override ──────────────────────────────────────────────────────

#[test]
fn log_file_with_json_content_sniffed_as_jsonl() {
    let f = file_with_ext(".log", "{\"level\":\"info\",\"msg\":\"started\"}\n");
    let h = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    assert_eq!(h.format.as_str(), "jsonl");
}

#[test]
fn log_file_with_json_array_stays_raw() {
    // JSON array (not JSONL) — starts with [ so sniff returns Raw
    let f = file_with_ext(".log", "[{\"a\":1},{\"b\":2}]\n");
    let h = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    assert_eq!(h.format.as_str(), "raw");
}

// ── CSV fields returned ───────────────────────────────────────────────────────

#[test]
fn csv_read_returns_fields() {
    let f = file_with_ext(".csv", "name,age,city\nalice,30,jakarta\n");
    let h = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    let lines = h.read_lines(0, 2).unwrap();
    assert!(lines[0].fields.is_some());
    assert_eq!(lines[0].fields.as_ref().unwrap(), &["name", "age", "city"]);
    assert_eq!(lines[1].fields.as_ref().unwrap(), &["alice", "30", "jakarta"]);
}

#[test]
fn jsonl_read_has_no_fields() {
    let f = file_with_ext(".jsonl", "{\"a\":1}\n");
    let h = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    let lines = h.read_lines(0, 1).unwrap();
    assert!(lines[0].fields.is_none());
}

#[test]
fn tsv_delimiter_is_tab() {
    let f = file_with_ext(".tsv", "a\tb\tc\n");
    let h = FileHandle::open(f.path().to_str().unwrap()).unwrap();
    let lines = h.read_lines(0, 1).unwrap();
    let fields = lines[0].fields.as_ref().unwrap();
    assert_eq!(fields, &["a", "b", "c"]);
}
