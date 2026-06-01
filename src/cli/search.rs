use anyhow::Result;
use serde::Serialize;
use crate::reader::stream::{stream_search, stream_search_regex};
use crate::security::validate_path;
use super::output::Format;

#[derive(Serialize)]
pub struct SearchOutput {
    pub query: String,
    pub results: Vec<SearchMatch>,
    pub truncated: bool,
}

#[derive(Serialize)]
pub struct SearchMatch {
    pub line: u64,
    pub content: String,
    pub match_start: usize,
    pub match_end: usize,
}

pub fn run(path: &str, query: &str, regex: bool, max: usize, fmt: Format) -> Result<()> {
    let path = validate_path(path)?;

    let (results, truncated) = if regex {
        stream_search_regex(&path, query, max)?
    } else {
        stream_search(&path, query, max)?
    };

    let out = SearchOutput {
        query: query.to_string(),
        results: results.iter().map(|r| SearchMatch {
            line: r.line_number + 1,
            content: r.content.clone(),
            match_start: r.match_start,
            match_end: r.match_end,
        }).collect(),
        truncated,
    };

    fmt.print(&out, |o| {
        if o.results.is_empty() {
            println!("No matches for {:?}", query);
            return;
        }
        for r in &o.results {
            let chars: Vec<char> = r.content.chars().collect();
            let before: String = chars[..r.match_start].iter().collect();
            let matched: String = chars[r.match_start..r.match_end].iter().collect();
            let after: String = chars[r.match_end..].iter().collect();
            println!("{:>7}  {}>>{}<<{}", r.line, before, matched, after);
        }
        if o.truncated {
            eprintln!("\n── {} matches shown (more exist, use --max to increase) ──", o.results.len());
        } else {
            eprintln!("\n── {} match(es) ──", o.results.len());
        }
    });

    Ok(())
}
