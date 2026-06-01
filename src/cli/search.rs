use anyhow::Result;
use serde::Serialize;
use crate::reader::{FileHandle, stream::{stream_search, stream_search_regex}};
use crate::security::validate_path;
use crate::protocol::response::ContextLine;
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub context_before: Vec<ContextLine>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub context_after: Vec<ContextLine>,
}

pub fn run(path: &str, query: &str, regex: bool, max: usize, before: usize, after: usize, fmt: Format) -> Result<()> {
    let path = validate_path(path)?;

    let (results, truncated) = if before > 0 || after > 0 {
        let h = FileHandle::open(&path)?;
        h.search_with_context(query, max, regex, before, after)?
    } else if regex {
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
            // Convert to 1-indexed to match SearchMatch.line convention
            context_before: r.context_before.iter().map(|c| ContextLine {
                line_number: c.line_number + 1,
                content: c.content.clone(),
            }).collect(),
            context_after: r.context_after.iter().map(|c| ContextLine {
                line_number: c.line_number + 1,
                content: c.content.clone(),
            }).collect(),
        }).collect(),
        truncated,
    };

    fmt.print(&out, |o| {
        if o.results.is_empty() {
            println!("No matches for {:?}", query);
            return;
        }
        for (i, r) in o.results.iter().enumerate() {
            if i > 0 { eprintln!("  ─────"); }

            // context before (line_number already 1-indexed in SearchMatch)
            for ctx in &r.context_before {
                println!("{:>7}  {}", ctx.line_number, ctx.content);
            }

            // match line with >>highlight<<
            let chars: Vec<char> = r.content.chars().collect();
            let before: String = chars[..r.match_start].iter().collect();
            let matched: String = chars[r.match_start..r.match_end].iter().collect();
            let after: String = chars[r.match_end..].iter().collect();
            println!("{:>7}▶ {}>>{}<<{}", r.line, before, matched, after);

            // context after (line_number already 1-indexed in SearchMatch)
            for ctx in &r.context_after {
                println!("{:>7}  {}", ctx.line_number, ctx.content);
            }
        }
        if o.truncated {
            eprintln!("\n── {} matches shown (more exist, use --max to increase) ──", o.results.len());
        } else {
            eprintln!("\n── {} match(es) ──", o.results.len());
        }
    });

    Ok(())
}
