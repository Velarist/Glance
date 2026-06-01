use anyhow::Result;
use serde::Serialize;
use crate::reader::FileHandle;
use crate::security::validate_path;
use super::output::Format;

#[derive(Serialize)]
pub struct InfoOutput {
    pub path: String,
    pub format: String,
    pub lines: u64,
    pub size_bytes: u64,
}

pub fn run(path: &str, fmt: Format) -> Result<()> {
    let path = validate_path(path)?;
    let h = FileHandle::open(&path)?;

    let out = InfoOutput {
        path: path.clone(),
        format: h.format.as_str().to_string(),
        lines: h.index.total_lines(),
        size_bytes: h.index.file_size(),
    };

    fmt.print(&out, |o| {
        let size_mb = o.size_bytes as f64 / 1024.0 / 1024.0;
        println!("File:   {}", o.path);
        println!("Format: {}", o.format);
        println!("Lines:  {}", o.lines);
        println!("Size:   {:.2} MB ({} bytes)", size_mb, o.size_bytes);
    });

    Ok(())
}
