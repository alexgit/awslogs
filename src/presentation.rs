use crate::log_fetcher::LogField;

#[derive(Default)]
pub struct FormattedResults {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

pub fn format_results(results: &[Vec<LogField>]) -> FormattedResults {
    if results.is_empty() {
        return FormattedResults::default();
    }
    let mut headers: Vec<String> = Vec::new();
    let mut formatted_rows: Vec<Vec<String>> = Vec::new();

    for row in results {
        let mut current_row: Vec<String> = Vec::new();
        let mut column_index = 0usize;

        for field in row {
            let label = field.name.as_deref().unwrap_or_default();
            if label == "@ptr" {
                continue;
            }

            if headers.len() <= column_index {
                let column_name = if label.is_empty() {
                    format!("Column {}", column_index + 1)
                } else {
                    label.to_string()
                };
                headers.push(column_name);
                for existing_row in &mut formatted_rows {
                    existing_row.push(String::new());
                }
            } else if !label.is_empty() {
                headers[column_index] = label.to_string();
            }

            current_row.push(field.value.clone());
            column_index += 1;
        }

        if column_index == 0 {
            continue;
        }

        if !headers.is_empty() {
            current_row.resize(headers.len(), String::new());
        }

        formatted_rows.push(current_row);
    }

    for row in &mut formatted_rows {
        row.resize(headers.len(), String::new());
    }

    if formatted_rows.is_empty() {
        FormattedResults::default()
    } else {
        FormattedResults {
            headers,
            rows: formatted_rows,
        }
    }
}

pub fn format_modal_value(value: &str) -> Vec<String> {
    if value.is_empty() {
        Vec::new()
    } else {
        value.lines().map(|line| line.to_string()).collect()
    }
}

pub fn format_modal_message(value: &str) -> Vec<String> {
    if value.trim().is_empty() {
        return Vec::new();
    }

    if let Some(pretty) = try_pretty_json(value) {
        return pretty.lines().map(|line| line.to_string()).collect();
    }

    format_modal_value(value)
}

fn try_pretty_json(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let starts_like_json = trimmed.starts_with('{') || trimmed.starts_with('[');
    if !starts_like_json {
        return None;
    }

    let mut result = String::new();
    let mut indent = 0usize;
    let mut in_string = false;
    let mut escape = false;

    for ch in trimmed.chars() {
        if escape {
            result.push(ch);
            escape = false;
            continue;
        }

        if ch == '\\' && in_string {
            result.push(ch);
            escape = true;
            continue;
        }

        if ch == '"' {
            in_string = !in_string;
            result.push(ch);
            continue;
        }

        if !in_string {
            match ch {
                '{' | '[' => {
                    result.push(ch);
                    result.push('\n');
                    indent += 1;
                    push_indent(&mut result, indent);
                    continue;
                }
                '}' | ']' => {
                    result.push('\n');
                    if indent > 0 {
                        indent -= 1;
                    }
                    push_indent(&mut result, indent);
                    result.push(ch);
                    continue;
                }
                ',' => {
                    result.push(ch);
                    result.push('\n');
                    push_indent(&mut result, indent);
                    continue;
                }
                ':' => {
                    result.push_str(": ");
                    continue;
                }
                c if c.is_whitespace() => {
                    continue;
                }
                _ => {}
            }
        }

        result.push(ch);
    }

    if in_string {
        return None;
    }

    Some(result.trim().to_string())
}

fn push_indent(buf: &mut String, indent: usize) {
    for _ in 0..indent {
        buf.push_str("  ");
    }
}
