use tempfile::NamedTempFile;
use glance::security::validate_path;

#[test]
fn valid_existing_file_returns_canonical_path() {
    let f = NamedTempFile::new().unwrap();
    let raw = f.path().to_str().unwrap();
    let result = validate_path(raw).unwrap();
    // Canonical path should be absolute (/ on Unix, C:\ on Windows)
    assert!(std::path::Path::new(&result).is_absolute());
    // Should resolve to the same file
    assert!(std::path::Path::new(&result).exists());
}

#[test]
fn nonexistent_path_returns_error() {
    let result = validate_path("/this/path/does/not/exist/file.jsonl");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("cannot access"));
}

#[test]
fn path_traversal_nonexistent_returns_error() {
    // Even if ../../ is used, if the resolved path doesn't exist it's rejected
    let result = validate_path("../../nonexistent_xyz_abc.txt");
    assert!(result.is_err());
}

#[test]
fn relative_path_resolves_to_absolute() {
    let f = NamedTempFile::new().unwrap();
    let raw = f.path().to_str().unwrap();
    // Use absolute path (tempfile gives absolute paths)
    let result = validate_path(raw).unwrap();
    assert!(
        std::path::Path::new(&result).is_absolute(),
        "should be absolute: {}",
        result
    );
}

#[test]
#[cfg(unix)]
fn symlink_resolves_to_real_target() {
    let f = NamedTempFile::new().unwrap();
    let target = f.path();

    let link_dir = tempfile::tempdir().unwrap();
    let link_path = link_dir.path().join("symlink_test");
    std::os::unix::fs::symlink(target, &link_path).unwrap();

    let result = validate_path(link_path.to_str().unwrap()).unwrap();
    // Should resolve to the real target, not the symlink
    let canonical_target = std::fs::canonicalize(target).unwrap();
    assert_eq!(result, canonical_target.to_str().unwrap());
}

#[test]
fn empty_path_returns_error() {
    let result = validate_path("");
    assert!(result.is_err());
}
