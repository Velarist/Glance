use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct OpenedData {
    pub file_id: u64,
    pub total_lines: u64,
    pub file_size: u64,
    pub format: String,
}

#[derive(Debug, Serialize)]
pub struct LinesData {
    pub lines: Vec<Line>,
    pub total_lines: u64,
    pub offset: u64,
}

#[derive(Debug, Serialize)]
pub struct Line {
    pub number: u64,
    pub content: String,
    /// Parsed CSV/TSV fields. Present only when format is csv/tsv.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct SearchResultsData {
    pub results: Vec<SearchResult>,
    pub total_found: usize,
    pub truncated: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct ContextLine {
    pub line_number: u64,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub line_number: u64,
    pub content: String,
    pub match_start: usize,
    pub match_end: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub context_before: Vec<ContextLine>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub context_after: Vec<ContextLine>,
}

#[derive(Debug, Serialize)]
pub struct CountData {
    pub count: u64,
}

#[derive(Debug, Serialize)]
pub struct InfoData {
    pub file_id: u64,
    pub path: String,
    pub total_lines: u64,
    pub file_size: u64,
    pub format: String,
}
