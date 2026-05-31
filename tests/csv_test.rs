use glance::reader::csv::{delimiter_for, parse_line};

// ── parse_line ────────────────────────────────────────────────────────────────

#[test]
fn simple_comma_separated() {
    let fields = parse_line("a,b,c", ',');
    assert_eq!(fields, vec!["a", "b", "c"]);
}

#[test]
fn quoted_field_with_comma_inside() {
    let fields = parse_line(r#"a,"b,c",d"#, ',');
    assert_eq!(fields, vec!["a", "b,c", "d"]);
}

#[test]
fn escaped_quote_inside_quoted_field() {
    let fields = parse_line(r#""say ""hello""","world""#, ',');
    assert_eq!(fields, vec!["say \"hello\"", "world"]);
}

#[test]
fn empty_fields() {
    let fields = parse_line("a,,c", ',');
    assert_eq!(fields, vec!["a", "", "c"]);
}

#[test]
fn single_field_no_delimiter() {
    let fields = parse_line("only", ',');
    assert_eq!(fields, vec!["only"]);
}

#[test]
fn tab_separated_values() {
    let fields = parse_line("one\ttwo\tthree", '\t');
    assert_eq!(fields, vec!["one", "two", "three"]);
}

#[test]
fn quoted_field_at_end() {
    let fields = parse_line(r#"first,"last field""#, ',');
    assert_eq!(fields, vec!["first", "last field"]);
}

#[test]
fn all_empty_fields() {
    let fields = parse_line(",,,", ',');
    assert_eq!(fields, vec!["", "", "", ""]);
}

// ── delimiter_for ─────────────────────────────────────────────────────────────

#[test]
fn csv_uses_comma() {
    assert_eq!(delimiter_for("data.csv"), ',');
    assert_eq!(delimiter_for("/path/to/file.csv"), ',');
}

#[test]
fn tsv_uses_tab() {
    assert_eq!(delimiter_for("data.tsv"), '\t');
    assert_eq!(delimiter_for("/path/to/file.tsv"), '\t');
}

#[test]
fn unknown_extension_defaults_to_comma() {
    assert_eq!(delimiter_for("file.txt"), ',');
}
