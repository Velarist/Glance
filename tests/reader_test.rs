use std::io::Write;
use tempfile::NamedTempFile;
use glance::reader::FileHandle;

fn tmp_jsonl(lines: &[&str]) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    for line in lines {
        writeln!(f, "{}", line).unwrap();
    }
    f.flush().unwrap();
    f
}

fn open(f: &NamedTempFile) -> FileHandle {
    let path = f.path().to_str().unwrap();
    // Remove any stale cache
    let _ = std::fs::remove_file(format!("{}.glance_idx", path));
    FileHandle::open(path).unwrap()
}

// ── read_lines ────────────────────────────────────────────────────────────────

#[test]
fn read_first_line() {
    let f = tmp_jsonl(&["hello", "world", "foo"]);
    let h = open(&f);
    let lines = h.read_lines(0, 1).unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].content, "hello");
    assert_eq!(lines[0].number, 0);
}

#[test]
fn read_all_lines() {
    let f = tmp_jsonl(&["a", "b", "c"]);
    let h = open(&f);
    let lines = h.read_lines(0, 10).unwrap();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].content, "a");
    assert_eq!(lines[2].content, "c");
}

#[test]
fn read_with_offset() {
    let f = tmp_jsonl(&["first", "second", "third"]);
    let h = open(&f);
    let lines = h.read_lines(1, 2).unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].content, "second");
    assert_eq!(lines[0].number, 1);
}

#[test]
fn read_limit_clamps_to_total() {
    let f = tmp_jsonl(&["x", "y"]);
    let h = open(&f);
    let lines = h.read_lines(0, 9999).unwrap();
    assert_eq!(lines.len(), 2);
}

#[test]
fn read_offset_out_of_range_returns_error() {
    let f = tmp_jsonl(&["only one line"]);
    let h = open(&f);
    assert!(h.read_lines(5, 10).is_err());
}

// ── search ────────────────────────────────────────────────────────────────────

#[test]
fn search_finds_matching_lines() {
    let f = tmp_jsonl(&["apple juice", "banana split", "apple pie"]);
    let h = open(&f);
    let (results, truncated) = h.search("apple", 100).unwrap();
    assert_eq!(results.len(), 2);
    assert!(!truncated);
    assert!(results.iter().all(|r| r.content.contains("apple")));
}

#[test]
fn search_is_case_insensitive() {
    let f = tmp_jsonl(&["Error: timeout", "WARNING: disk full", "error: retry"]);
    let h = open(&f);
    let (results, _) = h.search("error", 100).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn search_truncates_at_max_results() {
    let f = tmp_jsonl(&["match", "match", "match", "match", "match"]);
    let h = open(&f);
    let (results, truncated) = h.search("match", 3).unwrap();
    assert_eq!(results.len(), 3);
    assert!(truncated);
}

#[test]
fn search_no_matches_returns_empty() {
    let f = tmp_jsonl(&["foo", "bar", "baz"]);
    let h = open(&f);
    let (results, truncated) = h.search("xyz", 100).unwrap();
    assert!(results.is_empty());
    assert!(!truncated);
}

#[test]
fn search_match_positions_correct() {
    let f = tmp_jsonl(&["hello world"]);
    let h = open(&f);
    let (results, _) = h.search("world", 10).unwrap();
    assert_eq!(results.len(), 1);
    let r = &results[0];
    // "world" starts at char index 6 in "hello world"
    assert_eq!(&r.content[r.match_start..r.match_end], "world");
}

// ── search_regex ──────────────────────────────────────────────────────────────

#[test]
fn search_regex_finds_pattern() {
    let f = tmp_jsonl(&["user_id: 123", "user_id: 456", "name: alice"]);
    let h = open(&f);
    let (results, _) = h.search_regex(r"user_id: \d+", 100).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn search_regex_invalid_pattern_returns_error() {
    let f = tmp_jsonl(&["anything"]);
    let h = open(&f);
    assert!(h.search_regex(r"[invalid", 10).is_err());
}

// ── count ─────────────────────────────────────────────────────────────────────

#[test]
fn count_returns_correct_number() {
    let f = tmp_jsonl(&["hit", "miss", "hit", "hit"]);
    let h = open(&f);
    assert_eq!(h.count("hit").unwrap(), 3);
}

#[test]
fn count_zero_when_no_match() {
    let f = tmp_jsonl(&["foo", "bar"]);
    let h = open(&f);
    assert_eq!(h.count("xyz").unwrap(), 0);
}

#[test]
fn count_regex_works() {
    let f = tmp_jsonl(&["id: 1", "id: 22", "name: alice"]);
    let h = open(&f);
    assert_eq!(h.count_regex(r"id: \d+").unwrap(), 2);
}
