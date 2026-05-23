#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExtractedTag {
    pub(crate) content: String,
    pub(crate) repaired: bool,
}

pub(crate) fn extract_tag(raw: &str, tag: &str, known_tags: &[&str]) -> Option<ExtractedTag> {
    let bounds = find_tag_bounds(raw, tag, known_tags)?;
    Some(ExtractedTag {
        content: raw[bounds.content_start..bounds.content_end].to_string(),
        repaired: bounds.repaired,
    })
}

pub(crate) fn strip_structured_blocks(raw: &str, tags: &[&str], known_tags: &[&str]) -> String {
    let mut cleaned = raw.to_string();
    for tag in tags {
        cleaned = remove_tag_block(&cleaned, tag, known_tags);
    }
    cleaned
}

pub(crate) fn strip_json_code_fence(raw: &str) -> (String, bool) {
    let trimmed = raw.trim();
    if !trimmed.starts_with("```") {
        return (trimmed.to_string(), false);
    }
    let without_prefix = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```JSON"))
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim();
    let without_suffix = without_prefix
        .strip_suffix("```")
        .unwrap_or(without_prefix)
        .trim();
    (without_suffix.to_string(), true)
}

pub(crate) fn repair_json_shape(raw: &str) -> Option<String> {
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for ch in raw.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' | '[' if !in_string => stack.push(ch),
            '}' if !in_string => {
                if stack.pop() != Some('{') {
                    return None;
                }
            }
            ']' if !in_string => {
                if stack.pop() != Some('[') {
                    return None;
                }
            }
            _ => {}
        }
    }
    if in_string || stack.is_empty() {
        return None;
    }

    let mut repaired = raw.trim().to_string();
    while let Some(open) = stack.pop() {
        repaired.push(match open {
            '{' => '}',
            '[' => ']',
            _ => unreachable!(),
        });
    }
    Some(repaired)
}

pub(crate) fn partial_tool_signal_present(raw: &str) -> bool {
    raw.contains("\"tool_name\"")
        || raw.contains("\"name\"")
        || raw.contains("\"arguments\"")
        || raw.contains("\"args\"")
        || raw.contains("<tool_call>")
        || raw.contains("<function=")
}

pub(crate) fn classify_invalid_tool_calls(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "empty_tool_block".into();
    }
    if has_unterminated_json_string(trimmed) {
        return "unterminated_string_value".into();
    }
    if partial_tool_signal_present(trimmed) {
        return "partial_tool_call_shape".into();
    }
    "invalid_tool_calls_json".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TagBounds {
    start: usize,
    content_start: usize,
    content_end: usize,
    end: usize,
    repaired: bool,
}

fn remove_tag_block(raw: &str, tag: &str, known_tags: &[&str]) -> String {
    let Some(bounds) = find_tag_bounds(raw, tag, known_tags) else {
        return raw.to_string();
    };
    let mut cleaned = String::new();
    cleaned.push_str(raw[..bounds.start].trim_end());
    if !cleaned.is_empty() && bounds.end < raw.len() {
        cleaned.push_str("\n\n");
    }
    cleaned.push_str(raw[bounds.end..].trim_start());
    cleaned
}

fn find_tag_bounds(raw: &str, tag: &str, known_tags: &[&str]) -> Option<TagBounds> {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");
    let start = raw.find(&start_tag)?;
    let content_start = start + start_tag.len();
    let closing_start = raw[content_start..]
        .find(&end_tag)
        .map(|offset| content_start + offset);
    let next_tag_start = find_next_structured_tag_start(raw, content_start, known_tags);

    match (closing_start, next_tag_start) {
        (Some(end_start), Some(next_start)) if end_start > next_start => Some(TagBounds {
            start,
            content_start,
            content_end: next_start,
            end: next_start,
            repaired: true,
        }),
        (Some(end_start), _) => Some(TagBounds {
            start,
            content_start,
            content_end: end_start,
            end: end_start + end_tag.len(),
            repaired: false,
        }),
        (None, Some(next_start)) => Some(TagBounds {
            start,
            content_start,
            content_end: next_start,
            end: next_start,
            repaired: true,
        }),
        (None, None) => Some(TagBounds {
            start,
            content_start,
            content_end: raw.len(),
            end: raw.len(),
            repaired: true,
        }),
    }
}

fn find_next_structured_tag_start(raw: &str, offset: usize, known_tags: &[&str]) -> Option<usize> {
    known_tags
        .iter()
        .filter_map(|tag| {
            let start_tag = format!("<{tag}>");
            raw[offset..].find(&start_tag).map(|index| offset + index)
        })
        .min()
}

fn has_unterminated_json_string(raw: &str) -> bool {
    let mut in_string = false;
    let mut escaped = false;
    for ch in raw.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            _ => {}
        }
    }
    in_string
}
