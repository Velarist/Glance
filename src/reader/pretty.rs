//! JSON pretty-printing for JSONL content.
//! Falls back to raw string if parsing fails — never panics.

/// Pretty-print a single JSON line. Returns raw input on parse failure.
pub fn json(raw: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| raw.to_string()),
        Err(_) => raw.to_string(),
    }
}
