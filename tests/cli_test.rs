/// Black-box CLI tests — spawn the binary, check stdout/stderr/exit code.
/// Written without looking at implementation. Tests all subcommands from
/// a developer's perspective: what goes in, what should come out.

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn glance() -> Command {
    Command::cargo_bin("glance").unwrap()
}

fn tmp(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    // Remove stale cache
    let _ = std::fs::remove_file(format!("{}.glance_idx", f.path().to_str().unwrap()));
    f
}

// ── glance --version / --help ─────────────────────────────────────────────────

#[test]
fn version_flag_prints_version() {
    glance().arg("--version").assert().success()
        .stdout(predicate::str::contains("glance"))
        .stdout(predicate::str::is_match(r"\d+\.\d+\.\d+").unwrap());
}

#[test]
fn help_flag_shows_subcommands() {
    glance().arg("--help").assert().success()
        .stdout(predicate::str::contains("info"))
        .stdout(predicate::str::contains("read"))
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("count"));
}

#[test]
fn subcommand_help_works() {
    glance().args(["read", "--help"]).assert().success()
        .stdout(predicate::str::contains("--offset"))
        .stdout(predicate::str::contains("--limit"))
        .stdout(predicate::str::contains("--pretty"));
}

// ── glance info ───────────────────────────────────────────────────────────────

#[test]
fn info_shows_line_count() {
    let f = tmp("line one\nline two\nline three\n");
    glance().args(["info", f.path().to_str().unwrap()])
        .assert().success()
        .stdout(predicate::str::contains("Lines:  3"));
}

#[test]
fn info_shows_format_jsonl() {
    let mut f2 = tempfile::Builder::new().suffix(".jsonl").tempfile().unwrap();
    f2.write_all(b"{\"a\":1}\n{\"b\":2}\n").unwrap();
    glance().args(["info", f2.path().to_str().unwrap()])
        .assert().success()
        .stdout(predicate::str::contains("Format: jsonl"));
}

#[test]
fn info_shows_format_csv() {
    let mut f = tempfile::Builder::new().suffix(".csv").tempfile().unwrap();
    f.write_all(b"a,b,c\n1,2,3\n").unwrap();
    glance().args(["info", f.path().to_str().unwrap()])
        .assert().success()
        .stdout(predicate::str::contains("Format: csv"));
}

#[test]
fn info_empty_file_shows_zero_lines() {
    let f = tmp("");
    glance().args(["info", f.path().to_str().unwrap()])
        .assert().success()
        .stdout(predicate::str::contains("Lines:  0"));
}

#[test]
fn info_nonexistent_file_fails() {
    glance().args(["info", "/nonexistent/file/xyz.jsonl"])
        .assert().failure();
}

#[test]
fn info_path_traversal_fails() {
    glance().args(["info", "../../nonexistent_xyz"])
        .assert().failure();
}

#[test]
fn info_shows_file_path() {
    let f = tmp("data\n");
    glance().args(["info", f.path().to_str().unwrap()])
        .assert().success()
        .stdout(predicate::str::contains("File:"));
}

// ── glance read ───────────────────────────────────────────────────────────────

#[test]
fn read_default_shows_lines() {
    let f = tmp("first\nsecond\nthird\n");
    glance().args(["read", f.path().to_str().unwrap()])
        .assert().success()
        .stdout(predicate::str::contains("first"))
        .stdout(predicate::str::contains("second"));
}

#[test]
fn read_with_offset() {
    let f = tmp("line0\nline1\nline2\nline3\n");
    glance().args(["read", f.path().to_str().unwrap(), "--offset", "2", "--limit", "1"])
        .assert().success()
        .stdout(predicate::str::contains("line2"))
        .stdout(predicate::str::contains("3")); // line number shown as 3 (1-indexed)
}

#[test]
fn read_limit_clamps_to_file_end() {
    let f = tmp("a\nb\n");
    glance().args(["read", f.path().to_str().unwrap(), "--limit", "999"])
        .assert().success()
        .stdout(predicate::str::contains("a"))
        .stdout(predicate::str::contains("b"));
}

#[test]
fn read_offset_out_of_range_fails() {
    let f = tmp("only one line\n");
    glance().args(["read", f.path().to_str().unwrap(), "--offset", "999"])
        .assert().failure();
}

#[test]
fn read_empty_file_shows_message() {
    let f = tmp("");
    glance().args(["read", f.path().to_str().unwrap()])
        .assert().success()
        .stdout(predicate::str::contains("empty"));
}

#[test]
fn read_pretty_expands_json() {
    let mut f = tempfile::Builder::new().suffix(".jsonl").tempfile().unwrap();
    f.write_all(b"{\"name\":\"alice\",\"age\":30}\n").unwrap();
    glance().args(["read", f.path().to_str().unwrap(), "--pretty"])
        .assert().success()
        .stdout(predicate::str::contains("\"name\""))
        .stdout(predicate::str::contains("\"alice\""));
}

#[test]
fn read_shows_line_numbers() {
    let f = tmp("alpha\nbeta\n");
    glance().args(["read", f.path().to_str().unwrap()])
        .assert().success()
        .stdout(predicate::str::contains("1"))
        .stdout(predicate::str::contains("2"));
}

#[test]
fn read_nonexistent_file_fails() {
    glance().args(["read", "/no/such/file.txt"])
        .assert().failure();
}

// ── glance search ─────────────────────────────────────────────────────────────

#[test]
fn search_finds_match() {
    let f = tmp("hello world\nfoo bar\n");
    glance().args(["search", f.path().to_str().unwrap(), "hello"])
        .assert().success()
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn search_is_case_insensitive() {
    let f = tmp("Error: disk full\ninfo: ok\n");
    glance().args(["search", f.path().to_str().unwrap(), "ERROR"])
        .assert().success()
        .stdout(predicate::str::contains("Error"));
}

#[test]
fn search_no_match_says_no_matches() {
    let f = tmp("foo\nbar\n");
    glance().args(["search", f.path().to_str().unwrap(), "xyz"])
        .assert().success()
        .stdout(predicate::str::contains("No matches"));
}

#[test]
fn search_highlights_match() {
    let f = tmp("status: error occurred\n");
    glance().args(["search", f.path().to_str().unwrap(), "error"])
        .assert().success()
        .stdout(predicate::str::contains(">>error<<"));
}

#[test]
fn search_shows_line_number() {
    let f = tmp("miss\nmiss\nhit\n");
    glance().args(["search", f.path().to_str().unwrap(), "hit"])
        .assert().success()
        .stdout(predicate::str::contains("3")); // line 3
}

#[test]
fn search_regex_mode() {
    let f = tmp("id: 123\nid: abc\nid: 456\n");
    glance().args(["search", f.path().to_str().unwrap(), r"\d+", "--regex"])
        .assert().success()
        .stdout(predicate::str::contains("123"))
        .stdout(predicate::str::contains("456"));
}

#[test]
fn search_invalid_regex_fails() {
    let f = tmp("data\n");
    glance().args(["search", f.path().to_str().unwrap(), "[invalid", "--regex"])
        .assert().failure();
}

#[test]
fn search_max_limits_results() {
    let f = tmp("match\nmatch\nmatch\nmatch\nmatch\n");
    glance().args(["search", f.path().to_str().unwrap(), "match", "--max", "2"])
        .assert().success()
        .stderr(predicate::str::contains("more exist")); // truncation notice on stderr
}

#[test]
fn search_nonexistent_file_fails() {
    glance().args(["search", "/no/such/file.txt", "query"])
        .assert().failure();
}

// ── glance count ──────────────────────────────────────────────────────────────

#[test]
fn count_returns_correct_number() {
    let f = tmp("hit\nmiss\nhit\nhit\n");
    glance().args(["count", f.path().to_str().unwrap(), "hit"])
        .assert().success()
        .stdout(predicate::str::is_match("^3\n$").unwrap());
}

#[test]
fn count_zero_when_no_match() {
    let f = tmp("foo\nbar\n");
    glance().args(["count", f.path().to_str().unwrap(), "xyz"])
        .assert().success()
        .stdout(predicate::str::is_match("^0\n$").unwrap());
}

#[test]
fn count_is_case_insensitive() {
    let f = tmp("ERROR\nerror\nError\nok\n");
    glance().args(["count", f.path().to_str().unwrap(), "error"])
        .assert().success()
        .stdout(predicate::str::is_match("^3\n$").unwrap());
}

#[test]
fn count_regex_mode() {
    let f = tmp("id: 1\nid: 22\nname: alice\n");
    glance().args(["count", f.path().to_str().unwrap(), r"id: \d+", "--regex"])
        .assert().success()
        .stdout(predicate::str::is_match("^2\n$").unwrap());
}

#[test]
fn count_invalid_regex_fails() {
    let f = tmp("data\n");
    glance().args(["count", f.path().to_str().unwrap(), "[bad", "--regex"])
        .assert().failure();
}

#[test]
fn count_nonexistent_file_fails() {
    glance().args(["count", "/no/such/file.txt", "query"])
        .assert().failure();
}

// ── glance validate ──────────────────────────────────────────────────────────

#[test]
fn validate_all_valid_exits_zero() {
    let f = tmp("{\"a\":1}\n{\"b\":2}\n{\"c\":3}\n");
    glance().args(["validate", f.path().to_str().unwrap()])
        .assert().success()
        .stdout(predicate::str::contains("valid"));
}

#[test]
fn validate_invalid_line_exits_one() {
    let f = tmp("{\"a\":1}\nNOT JSON\n{\"c\":3}\n");
    glance().args(["validate", f.path().to_str().unwrap()])
        .assert().failure() // exit code 1
        .stdout(predicate::str::contains("invalid"))
        .stdout(predicate::str::contains("NOT JSON"));
}

#[test]
fn validate_reports_correct_line_number() {
    let f = tmp("{\"ok\":1}\n{\"ok\":2}\nBAD\n{\"ok\":4}\n");
    glance().args(["validate", f.path().to_str().unwrap()])
        .assert().failure()
        .stdout(predicate::str::contains("3")); // line 3 is invalid
}

#[test]
fn validate_empty_file_exits_zero() {
    let f = tmp("");
    glance().args(["validate", f.path().to_str().unwrap()])
        .assert().success();
}

#[test]
fn validate_nonexistent_file_fails() {
    glance().args(["validate", "/no/such/file.jsonl"])
        .assert().failure();
}

// ── --json flag ───────────────────────────────────────────────────────────────

#[test]
fn info_json_output_is_valid_json() {
    let f = tmp("line one\nline two\n");
    let out = glance().args(["info", f.path().to_str().unwrap(), "--json"])
        .assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("must be valid JSON");
    assert_eq!(v["lines"], 2);
    assert!(v["format"].is_string());
    assert!(v["size_bytes"].is_number());
}

#[test]
fn search_json_output_has_results_array() {
    let f = tmp("hit line\nmiss\nhit again\n");
    let out = glance().args(["search", f.path().to_str().unwrap(), "hit", "--json"])
        .assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("must be valid JSON");
    assert!(v["results"].is_array());
    assert_eq!(v["results"].as_array().unwrap().len(), 2);
    assert_eq!(v["truncated"], false);
}

#[test]
fn count_json_output_has_count_field() {
    let f = tmp("hit\nmiss\nhit\n");
    let out = glance().args(["count", f.path().to_str().unwrap(), "hit", "--json"])
        .assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("must be valid JSON");
    assert_eq!(v["count"], 2);
}

#[test]
fn validate_json_output_has_invalid_array() {
    let f = tmp("{\"ok\":1}\nBAD\n{\"ok\":3}\n");
    let out = glance().args(["validate", f.path().to_str().unwrap(), "--json"])
        .assert().failure().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("must be valid JSON");
    assert_eq!(v["invalid_count"], 1);
    assert_eq!(v["total_lines"], 3);
    assert_eq!(v["invalid"][0]["line"], 2);
}

#[test]
fn read_json_output_has_lines_array() {
    let f = tmp("alpha\nbeta\ngamma\n");
    let out = glance().args(["read", f.path().to_str().unwrap(), "--limit", "3", "--json"])
        .assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("must be valid JSON");
    assert!(v["lines"].is_array());
    assert_eq!(v["lines"].as_array().unwrap().len(), 3);
    assert_eq!(v["lines"][0]["content"], "alpha");
}

// ── glance serve ─────────────────────────────────────────────────────────────

#[test]
fn serve_subcommand_exists() {
    glance().arg("serve").write_stdin("").assert().success();
}
