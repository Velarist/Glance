/// Black-box tests for search and count — edge cases, Unicode,
/// boundary positions, regex features.

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

// ── Match position accuracy ───────────────────────────────────────────────────

#[test]
fn search_match_at_start_of_line() {
    let f = file("error: disk full\ninfo: ok\n");
    let h = open(&f);
    let (results, _) = h.search("error", 10).unwrap();
    assert_eq!(results[0].match_start, 0);
    assert_eq!(&results[0].content[results[0].match_start..results[0].match_end], "error");
}

#[test]
fn search_match_at_end_of_line() {
    let f = file("status: error\n");
    let h = open(&f);
    let (results, _) = h.search("error", 10).unwrap();
    let r = &results[0];
    assert_eq!(&r.content[r.match_start..r.match_end], "error");
}

#[test]
fn search_match_on_first_line() {
    let f = file("FOUND\nmiss\nmiss\n");
    let h = open(&f);
    let (results, _) = h.search("found", 10).unwrap();
    assert_eq!(results[0].line_number, 0);
}

#[test]
fn search_match_on_last_line() {
    let f = file("miss\nmiss\nFOUND\n");
    let h = open(&f);
    let (results, _) = h.search("found", 10).unwrap();
    assert_eq!(results[0].line_number, 2);
}

// ── Case sensitivity ──────────────────────────────────────────────────────────

#[test]
fn search_uppercase_query_finds_lowercase_content() {
    let f = file("error occurred\n");
    let h = open(&f);
    let (results, _) = h.search("ERROR", 10).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn search_mixed_case_query() {
    let f = file("TypeError: null\n");
    let h = open(&f);
    let (results, _) = h.search("typeerror", 10).unwrap();
    assert_eq!(results.len(), 1);
}

// ── Unicode ───────────────────────────────────────────────────────────────────

#[test]
fn search_unicode_content_ascii_query() {
    let f = file("café error\nbar\n");
    let h = open(&f);
    let (results, _) = h.search("error", 10).unwrap();
    assert_eq!(results.len(), 1);
    // match_start/end should be char indices, verify roundtrip
    let r = &results[0];
    let chars: Vec<char> = r.content.chars().collect();
    let matched: String = chars[r.match_start..r.match_end].iter().collect();
    assert_eq!(matched.to_lowercase(), "error");
}

#[test]
fn search_unicode_query() {
    let f = file("hello wörld\ngoodbye\n");
    let h = open(&f);
    let (results, _) = h.search("wörld", 10).unwrap();
    assert_eq!(results.len(), 1);
}

// ── Truncation ────────────────────────────────────────────────────────────────

#[test]
fn search_truncation_flag_correct() {
    let f = file("match\nmatch\nmatch\nmatch\nmatch\n");
    let h = open(&f);
    let (results, truncated) = h.search("match", 3).unwrap();
    assert_eq!(results.len(), 3);
    assert!(truncated);
}

#[test]
fn search_no_truncation_when_under_limit() {
    let f = file("match\nmatch\nmiss\n");
    let h = open(&f);
    let (results, truncated) = h.search("match", 100).unwrap();
    assert_eq!(results.len(), 2);
    assert!(!truncated);
}

// ── Empty query validation ────────────────────────────────────────────────────

#[test]
fn search_empty_query_returns_error() {
    let f = file("some content\n");
    let h = open(&f);
    assert!(h.search("", 10).is_err());
}

#[test]
fn count_empty_query_returns_error() {
    let f = file("some content\n");
    let h = open(&f);
    assert!(h.count("").is_err());
}

// ── Regex features ────────────────────────────────────────────────────────────

#[test]
fn regex_anchored_start() {
    let f = file("error: bad\nsomething error\n");
    let h = open(&f);
    let (results, _) = h.search_regex("^error", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].line_number, 0);
}

#[test]
fn regex_anchored_end() {
    let f = file("status ok\nstatus error\n");
    let h = open(&f);
    let (results, _) = h.search_regex("error$", 10).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn regex_digit_pattern() {
    let f = file("id: 123\nid: abc\nid: 456\n");
    let h = open(&f);
    let (results, _) = h.search_regex(r"id: \d+", 10).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn regex_alternation() {
    let f = file("warn: low disk\nerror: crash\ninfo: ok\n");
    let h = open(&f);
    let (results, _) = h.search_regex("warn|error", 10).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn regex_invalid_returns_error_not_panic() {
    let f = file("data\n");
    let h = open(&f);
    assert!(h.search_regex("[unclosed", 10).is_err());
    assert!(h.search_regex("(?P<", 10).is_err());
}

#[test]
fn regex_match_positions_correct() {
    let f = file("user_id: 42\n");
    let h = open(&f);
    let (results, _) = h.search_regex(r"\d+", 10).unwrap();
    let r = &results[0];
    let chars: Vec<char> = r.content.chars().collect();
    let matched: String = chars[r.match_start..r.match_end].iter().collect();
    assert_eq!(matched, "42");
}

// ── Count accuracy ────────────────────────────────────────────────────────────

#[test]
fn count_all_lines_match() {
    let f = file("x\nx\nx\n");
    let h = open(&f);
    assert_eq!(h.count("x").unwrap(), 3);
}

#[test]
fn count_partial_match() {
    let f = file("hit\nmiss\nhit\nmiss\nhit\n");
    let h = open(&f);
    assert_eq!(h.count("hit").unwrap(), 3);
}

#[test]
fn count_is_case_insensitive() {
    let f = file("ERROR\nerror\nError\nok\n");
    let h = open(&f);
    assert_eq!(h.count("error").unwrap(), 3);
}

#[test]
fn count_regex_zero() {
    let f = file("abc\ndef\n");
    let h = open(&f);
    assert_eq!(h.count_regex(r"\d+").unwrap(), 0);
}
