use anyhow::Result;
use serde::Serialize;
use crate::reader::stream::{stream_count, stream_count_regex};
use crate::security::validate_path;
use super::output::Format;

#[derive(Serialize)]
pub struct CountOutput {
    pub query: String,
    pub count: u64,
}

pub fn run(path: &str, query: &str, regex: bool, fmt: Format) -> Result<()> {
    let path = validate_path(path)?;

    let n = if regex {
        stream_count_regex(&path, query)?
    } else {
        stream_count(&path, query)?
    };

    let out = CountOutput { query: query.to_string(), count: n };

    fmt.print(&out, |o| println!("{}", o.count));

    Ok(())
}
