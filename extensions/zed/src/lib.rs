use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use zed_extension_api::{
    self as zed, Range, Result, SlashCommand, SlashCommandArgumentCompletion,
    SlashCommandOutput, SlashCommandOutputSection,
};

// ── Line index (built in WASM via std::fs — WASI allows filesystem access) ───

struct LineIndex {
    offsets: Vec<u64>,
    file_size: u64,
}

impl LineIndex {
    fn build(path: &str) -> Result<Self> {
        let file = File::open(path).map_err(|e| format!("cannot open '{path}': {e}"))?;
        let file_size = file
            .metadata()
            .map_err(|e| e.to_string())?
            .len();
        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut offsets = vec![0u64];
        let mut pos: u64 = 0;
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let n = reader
                .read_until(b'\n', &mut buf)
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            pos += n as u64;
            if pos < file_size {
                offsets.push(pos);
            }
        }
        Ok(Self { offsets, file_size })
    }

    fn total_lines(&self) -> u64 {
        self.offsets.len() as u64
    }

    fn offset_of(&self, line: u64) -> Option<u64> {
        self.offsets.get(line as usize).copied()
    }
}

// ── File operations ───────────────────────────────────────────────────────────

struct LineResult {
    number: u64,
    content: String,
}

fn read_lines(path: &str, index: &LineIndex, offset: u64, limit: u64) -> Result<Vec<LineResult>> {
    let total = index.total_lines();
    if offset >= total {
        return Err(format!("offset {offset} out of range (file has {total} lines)"));
    }
    let start = offset;
    let end = (start + limit).min(total);
    let byte_offset = index.offset_of(start).unwrap_or(0);

    let mut file = File::open(path).map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(byte_offset))
        .map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut buf = String::new();

    for line_num in start..end {
        buf.clear();
        let n = reader.read_line(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        lines.push(LineResult {
            number: line_num,
            content: buf.trim_end_matches('\n').trim_end_matches('\r').to_string(),
        });
    }
    Ok(lines)
}

struct SearchResult {
    line_number: u64,
    content: String,
    match_start: usize,
    match_end: usize,
}

fn search_file(
    path: &str,
    query: &str,
    max_results: usize,
) -> Result<(Vec<SearchResult>, bool)> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::with_capacity(64 * 1024, file);
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        let line_lower = line.to_lowercase();
        if let Some(pos) = line_lower.find(&query_lower) {
            results.push(SearchResult {
                line_number: line_num as u64,
                content: line,
                match_start: pos,
                match_end: pos + query.len(),
            });
            if results.len() >= max_results {
                return Ok((results, true));
            }
        }
    }
    Ok((results, false))
}

fn count_matches(path: &str, query: &str) -> Result<u64> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::with_capacity(64 * 1024, file);
    let query_lower = query.to_lowercase();
    let mut count = 0u64;
    for line in reader.lines() {
        if line.map_err(|e| e.to_string())?.to_lowercase().contains(&query_lower) {
            count += 1;
        }
    }
    Ok(count)
}

// ── Argument parsing ──────────────────────────────────────────────────────────

struct Args {
    path: String,
    line: Option<u64>,
    search: Option<String>,
    count_query: Option<String>,
    info: bool,
}

impl Args {
    fn parse(arguments: &[String]) -> Result<Self> {
        if arguments.is_empty() {
            return Err(
                "usage: /glance <path> [--line N] [--search query] [--count query] [--info]"
                    .to_string(),
            );
        }
        let mut path = arguments[0].clone();
        let mut line = None;
        let mut search = None;
        let mut count_query = None;
        let mut info = false;
        let mut i = 1;
        while i < arguments.len() {
            match arguments[i].as_str() {
                "--line" | "-l" => {
                    i += 1;
                    line = Some(
                        arguments.get(i).ok_or("--line requires a number")?
                            .parse::<u64>().map_err(|_| "--line must be a positive integer")?,
                    );
                }
                "--search" | "-s" => {
                    i += 1;
                    search = Some(arguments.get(i).ok_or("--search requires a query")?.clone());
                }
                "--count" | "-c" => {
                    i += 1;
                    count_query = Some(arguments.get(i).ok_or("--count requires a query")?.clone());
                }
                "--info" | "-i" => info = true,
                other => path = other.to_string(),
            }
            i += 1;
        }
        Ok(Args { path, line, search, count_query, info })
    }
}

// ── Output formatting ─────────────────────────────────────────────────────────

fn format_info(path: &str, index: &LineIndex) -> String {
    let ext = path.rsplit('.').next().unwrap_or("?");
    let fmt = match ext {
        "jsonl" | "ndjson" => "jsonl",
        "csv" => "csv",
        "tsv" => "tsv",
        _ => "raw",
    };
    let size_mb = index.file_size as f64 / 1024.0 / 1024.0;
    format!(
        "**{}**\n- Format: `{}`\n- Lines: `{}`\n- Size: `{:.1} MB`",
        path, fmt, index.total_lines(), size_mb
    )
}

fn format_lines(path: &str, lines: &[LineResult], offset: u64, total: u64) -> String {
    if lines.is_empty() {
        return format!("No lines returned (offset {offset}, total {total}).");
    }
    let end = offset + lines.len() as u64;
    let lang = if path.ends_with(".jsonl") || path.ends_with(".ndjson") { "json" } else { "" };
    let mut out = format!(
        "**{}** — lines {}-{} of {}\n\n```{}\n",
        path, offset + 1, end, total, lang
    );
    for l in lines {
        out.push_str(&format!("{:>6}  {}\n", l.number + 1, l.content));
    }
    out.push_str("```");
    out
}

fn format_search(path: &str, query: &str, results: &[SearchResult], truncated: bool) -> String {
    if results.is_empty() {
        return format!("No matches for `{}` in `{}`.", query, path);
    }
    let suffix = if truncated {
        format!(" *(first {} shown)*", results.len())
    } else {
        String::new()
    };
    let mut out = format!(
        "**{}** matches for `{}`{}\n\n```\n",
        results.len(), query, suffix
    );
    for r in results {
        out.push_str(&format!("{:>6}  {}\n", r.line_number + 1, r.content));
    }
    out.push_str("```");
    out
}

// ── Extension ─────────────────────────────────────────────────────────────────

struct GlanceExtension;

impl zed::Extension for GlanceExtension {
    fn new() -> Self {
        GlanceExtension
    }

    fn complete_slash_command_argument(
        &self,
        _command: SlashCommand,
        _arguments: Vec<String>,
    ) -> Result<Vec<SlashCommandArgumentCompletion>> {
        Ok(vec![
            SlashCommandArgumentCompletion {
                label: "--search <query>".to_string(),
                new_text: "--search ".to_string(),
                run_command: false,
            },
            SlashCommandArgumentCompletion {
                label: "--line <n>".to_string(),
                new_text: "--line ".to_string(),
                run_command: false,
            },
            SlashCommandArgumentCompletion {
                label: "--count <query>".to_string(),
                new_text: "--count ".to_string(),
                run_command: false,
            },
            SlashCommandArgumentCompletion {
                label: "--info".to_string(),
                new_text: "--info".to_string(),
                run_command: true,
            },
        ])
    }

    fn run_slash_command(
        &self,
        _command: SlashCommand,
        arguments: Vec<String>,
        _worktree: Option<&zed::Worktree>,
    ) -> Result<SlashCommandOutput> {
        let args = Args::parse(&arguments)?;

        let text = if let Some(ref query) = args.search {
            let (results, truncated) = search_file(&args.path, query, 50)?;
            format_search(&args.path, query, &results, truncated)

        } else if let Some(ref query) = args.count_query {
            let count = count_matches(&args.path, query)?;
            format!("`{}` matches for `{}` in `{}`", count, query, args.path)

        } else {
            // For info and read we need the line index
            let index = LineIndex::build(&args.path)?;

            if args.info {
                format_info(&args.path, &index)
            } else {
                let offset = args.line.map(|n| n.saturating_sub(1)).unwrap_or(0);
                let lines = read_lines(&args.path, &index, offset, 50)?;
                format_lines(&args.path, &lines, offset, index.total_lines())
            }
        };

        Ok(SlashCommandOutput {
            sections: vec![SlashCommandOutputSection {
                range: Range { start: 0, end: text.len() as u32 },
                label: format!("glance: {}", args.path),
            }],
            text,
        })
    }
}

zed::register_extension!(GlanceExtension);
