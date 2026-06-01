use anyhow::Result;
use serde::Serialize;
use crate::reader::{FileHandle, stream::read_lines_direct};
use crate::security::validate_path;
use super::output::Format;

#[derive(Serialize)]
pub struct ReadOutput {
    pub total_lines: u64,
    pub offset: u64,
    pub lines: Vec<ReadLine>,
}

#[derive(Serialize)]
pub struct ReadLine {
    pub number: u64,
    pub content: String,
}

pub fn run(path: &str, offset: u64, limit: u64, pretty: bool, fmt: Format) -> Result<()> {
    let path = validate_path(path)?;
    let h = FileHandle::open(&path)?;
    let total = h.index.total_lines();

    if total == 0 {
        match fmt {
            Format::Json => println!("{{\"total_lines\":0,\"offset\":0,\"lines\":[]}}"),
            Format::Human => println!("(empty file)"),
        }
        return Ok(());
    }
    if offset >= total {
        anyhow::bail!("offset {} out of range (file has {} lines)", offset, total);
    }

    let end = (offset + limit).min(total);
    let byte_offset = h.index.line_offset(offset)
        .ok_or_else(|| anyhow::anyhow!("index error at offset {}", offset))?;

    let lines = read_lines_direct(&path, byte_offset, offset, end, pretty, h.format)?;

    let out = ReadOutput {
        total_lines: total,
        offset,
        lines: lines.iter().map(|l| ReadLine {
            number: l.number,
            content: l.content.clone(),
        }).collect(),
    };

    fmt.print(&out, |o| {
        for l in &o.lines {
            println!("{:>7}  {}", l.number + 1, l.content);
        }
        eprintln!("\n── Lines {}-{} of {} ──", offset + 1, end, total);
    });

    Ok(())
}
