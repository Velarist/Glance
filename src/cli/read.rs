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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<String>>,
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

    let format = h.format;
    let lines = read_lines_direct(&path, byte_offset, offset, end, pretty, format)?;

    let out = ReadOutput {
        total_lines: total,
        offset,
        lines: lines.iter().map(|l| ReadLine {
            number: l.number,
            content: l.content.clone(),
            fields: l.fields.clone(), // ← fields preserved, not dropped
        }).collect(),
    };

    fmt.print(&out, |o| {
        let has_fields = o.lines.first().and_then(|l| l.fields.as_ref()).is_some();

        if has_fields {
            render_table(o);
        } else {
            for l in &o.lines {
                println!("{:>7}  {}", l.number + 1, l.content);
            }
        }
        eprintln!("\n── Lines {}-{} of {} ──", offset + 1, end, total);
    });

    Ok(())
}

fn render_table(out: &ReadOutput) {
    // Compute column widths across all rows
    let num_cols = out.lines.first()
        .and_then(|l| l.fields.as_ref())
        .map(|f| f.len())
        .unwrap_or(0);

    let mut widths = vec![0usize; num_cols];
    for l in &out.lines {
        if let Some(fields) = &l.fields {
            for (i, f) in fields.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(f.len());
                }
            }
        }
    }

    // Header row — line number column
    let line_col_w = 7;
    let separator: String = widths.iter()
        .map(|w| "-".repeat(w + 2))
        .collect::<Vec<_>>()
        .join("+");
    eprintln!("{}", separator);

    for l in &out.lines {
        if let Some(fields) = &l.fields {
            let row: String = fields.iter().enumerate()
                .map(|(i, f)| format!(" {:width$} ", f, width = widths.get(i).copied().unwrap_or(0)))
                .collect::<Vec<_>>()
                .join("|");
            println!("{:>width$}  |{}|", l.number + 1, row, width = line_col_w);
        }
    }

    eprintln!("{}", separator);
}
