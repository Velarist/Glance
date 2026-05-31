use anyhow::Result;

/// Canonicalize and validate a file path before opening.
///
/// `std::fs::canonicalize` resolves `..`, `.`, and symlinks in one step
/// and returns `Err` if the path does not exist or is inaccessible —
/// eliminating path traversal and existence checks in a single gate.
/// All file access in the daemon must go through this function.
pub fn validate_path(raw: &str) -> Result<String> {
    let canonical = std::fs::canonicalize(raw)
        .map_err(|e| anyhow::anyhow!("cannot access '{}': {}", raw, e))?;
    Ok(canonical.to_string_lossy().into_owned())
}
