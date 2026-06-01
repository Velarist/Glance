use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct OpenParams {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct ReadParams {
    pub file_id: u64,
    /// Line number to start from (0-indexed)
    pub offset: u64,
    /// How many lines to return
    pub limit: u64,
    /// If true and format is JSONL, pretty-print each line's JSON content.
    pub pretty: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub file_id: u64,
    pub query: String,
    pub max_results: Option<usize>,
    /// If true, `query` is treated as a regex pattern (case-sensitive).
    /// If false or omitted, plain case-insensitive substring match is used.
    pub regex: Option<bool>,
    /// Number of lines to include before each match.
    pub before: Option<usize>,
    /// Number of lines to include after each match.
    pub after: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct InfoParams {
    pub file_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct CloseParams {
    pub file_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct CountParams {
    pub file_id: u64,
    pub query: String,
    pub regex: Option<bool>,
}
