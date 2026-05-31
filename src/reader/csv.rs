/// Parse one CSV/TSV line into fields, respecting RFC 4180 quoting rules.
/// Handles: quoted fields, commas/tabs inside quotes, escaped quotes (`""`).
pub fn parse_line(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            c if c == delimiter && !in_quotes => {
                fields.push(std::mem::take(&mut field));
            }
            c => field.push(c),
        }
    }
    fields.push(field);
    fields
}

pub fn delimiter_for(path: &str) -> char {
    if path.ends_with(".tsv") { '\t' } else { ',' }
}
