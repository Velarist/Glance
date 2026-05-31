use std::io::Write;
use tempfile::NamedTempFile;
use glance::index::line_index::LineIndex;

fn tmp(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    f
}

#[test]
fn empty_file_has_zero_lines() {
    let f = tmp("");
    let idx = LineIndex::build(f.path().to_str().unwrap()).unwrap();
    assert_eq!(idx.total_lines(), 0);
}

#[test]
fn single_line_no_trailing_newline() {
    let f = tmp("hello world");
    let idx = LineIndex::build(f.path().to_str().unwrap()).unwrap();
    assert_eq!(idx.total_lines(), 1);
    assert_eq!(idx.line_offset(0), Some(0));
    assert_eq!(idx.line_offset(1), None);
}

#[test]
fn single_line_with_trailing_newline() {
    let f = tmp("hello world\n");
    let idx = LineIndex::build(f.path().to_str().unwrap()).unwrap();
    assert_eq!(idx.total_lines(), 1);
}

#[test]
fn three_lines_correct_offsets() {
    let f = tmp("aaa\nbb\nc\n");
    let idx = LineIndex::build(f.path().to_str().unwrap()).unwrap();
    assert_eq!(idx.total_lines(), 3);
    assert_eq!(idx.line_offset(0), Some(0));   // "aaa\n" starts at 0
    assert_eq!(idx.line_offset(1), Some(4));   // "bb\n" starts at 4
    assert_eq!(idx.line_offset(2), Some(7));   // "c\n" starts at 7
    assert_eq!(idx.line_offset(3), None);
}

#[test]
fn windows_line_endings_correct_offsets() {
    let f = tmp("aaa\r\nbb\r\n");
    let idx = LineIndex::build(f.path().to_str().unwrap()).unwrap();
    assert_eq!(idx.total_lines(), 2);
    assert_eq!(idx.line_offset(0), Some(0));
    assert_eq!(idx.line_offset(1), Some(5)); // "aaa\r\n" = 5 bytes
}

#[test]
fn from_parts_round_trip() {
    let f = tmp("line1\nline2\nline3\n");
    let original = LineIndex::build(f.path().to_str().unwrap()).unwrap();
    let restored = LineIndex::from_parts(original.offsets().to_vec(), original.file_size());
    assert_eq!(original.total_lines(), restored.total_lines());
    assert_eq!(original.file_size(), restored.file_size());
    for i in 0..original.total_lines() {
        assert_eq!(original.line_offset(i), restored.line_offset(i));
    }
}
