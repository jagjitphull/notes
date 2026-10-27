use serde::{Deserialize, Serialize};

/// The YAML front matter every note file carries. `title` is deliberately
/// not stored here: like Apple Notes, the first line of the body *is* the
/// title, so there is only one place a note's title can drift out of sync.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FrontMatter {
    pub id: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pinned: bool,
    pub created_at: String,
    #[serde(default)]
    pub deleted_at: Option<String>,
    /// A note usable as the starting point for a new note (see
    /// store::create_note_from_template), kept out of normal folder/"All
    /// Notes" views the same way a deleted note is.
    #[serde(default)]
    pub is_template: bool,
}

pub struct ParsedNote {
    pub front_matter: Option<FrontMatter>,
    pub body: String,
}

/// Splits `---`-delimited YAML front matter from the Markdown body. Files
/// without a recognizable front matter block (e.g. a plain .md file dropped
/// in from outside the app) come back with `front_matter: None` and the
/// whole file as `body`; the caller is expected to assign a fresh id and
/// rewrite the file so it's tracked going forward.
pub fn parse(raw: &str) -> ParsedNote {
    let mut lines = raw.lines();

    if lines.next() != Some("---") {
        return ParsedNote {
            front_matter: None,
            body: raw.to_string(),
        };
    }

    let mut yaml_lines = Vec::new();
    let mut closed = false;
    let mut consumed = "---\n".len();
    for line in lines.by_ref() {
        consumed += line.len() + 1;
        if line == "---" {
            closed = true;
            break;
        }
        yaml_lines.push(line);
    }

    if !closed {
        return ParsedNote {
            front_matter: None,
            body: raw.to_string(),
        };
    }

    let yaml = yaml_lines.join("\n");
    let front_matter: Option<FrontMatter> = serde_yaml::from_str(&yaml).ok();

    let body = raw
        .get(consumed.min(raw.len())..)
        .unwrap_or("")
        .trim_start_matches('\n')
        .to_string();

    ParsedNote { front_matter, body }
}

pub fn serialize(front_matter: &FrontMatter, body: &str) -> String {
    let yaml = serde_yaml::to_string(front_matter).expect("FrontMatter always serializes");
    format!("---\n{yaml}---\n{body}")
}

/// Strips a single leading Markdown block marker (heading, blockquote,
/// bullet/task/ordered list item) so a rich-text title reads as plain text
/// — e.g. "## Groceries" or "- [ ] Groceries" both become "Groceries".
fn strip_markdown_prefix(line: &str) -> String {
    let mut s = line.trim_start();

    if let Some(rest) = s.strip_prefix("> ") {
        s = rest.trim_start();
    }

    let hash_count = s.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hash_count)
        && let Some(rest) = s[hash_count..].strip_prefix(' ')
    {
        s = rest;
    }

    for bullet in ['-', '*', '+'] {
        for marker in [" [ ] ", " [x] ", " [X] "] {
            let prefix = format!("{bullet}{marker}");
            if let Some(rest) = s.strip_prefix(&prefix) {
                return rest.trim().to_string();
            }
        }
    }

    for bullet in ['-', '*', '+'] {
        let prefix = format!("{bullet} ");
        if let Some(rest) = s.strip_prefix(&prefix) {
            s = rest;
            break;
        }
    }

    let digit_count = s.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_count > 0 {
        let after = &s[digit_count..];
        if let Some(rest) = after
            .strip_prefix(". ")
            .or_else(|| after.strip_prefix(") "))
        {
            s = rest;
        }
    }

    s.trim().to_string()
}

/// Strips the trailing `\` that Markdown serializers (ours included) write
/// at the end of a line to mark it as a hard break, e.g. `"Title\\"` — a
/// rendering artifact, not content.
fn strip_hard_break_marker(line: &str) -> &str {
    line.strip_suffix('\\').unwrap_or(line)
}

/// Replaces every `![alt](url)` image span with a short `[image]`
/// placeholder. Pasted/dropped images are inlined as base64 data URIs
/// directly in the note body (see Editor.vue) rather than saved to disk
/// like other attachments, so a single screenshot can otherwise balloon
/// this text to several MB — which then gets shipped over IPC for every
/// note on every list refresh, regex-scanned for `[[wikilinks]]` by the
/// graph view, and indexed (uselessly) by FTS5. This runs on the title and
/// preview text only; the saved `.md` file itself is untouched.
fn strip_embedded_images(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'!'
            && bytes.get(i + 1) == Some(&b'[')
            && let Some(close_bracket) = text[i + 2..].find(']')
            && bytes.get(i + 2 + close_bracket + 1) == Some(&b'(')
            && let Some(close_paren) = text[i + 2 + close_bracket + 2..].find(')')
        {
            out.push_str("[image]");
            i = i + 2 + close_bracket + 2 + close_paren + 1;
            continue;
        }
        // One char (not byte) at a time, to stay on UTF-8 boundaries.
        let ch_len = text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        out.push_str(&text[i..i + ch_len]);
        i += ch_len;
    }
    out
}

/// The first non-empty line of the body (Markdown syntax stripped), used as
/// both the note's display title and the basis for its filename.
pub fn extract_title(body: &str) -> String {
    let raw = body.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    strip_embedded_images(&strip_markdown_prefix(strip_hard_break_marker(raw)))
}

/// The remaining lines after the title, used for the list-view preview,
/// full-text search index, and the graph view's link extraction.
pub fn extract_preview(body: &str) -> String {
    let mut lines = body.lines().skip_while(|l| l.trim().is_empty());
    lines.next(); // skip the title line itself
    let joined = lines
        .map(strip_hard_break_marker)
        .collect::<Vec<_>>()
        .join("\n");
    strip_embedded_images(joined.trim())
}

pub fn sanitize_filename(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "New Note".to_string()
    } else {
        trimmed.chars().take(120).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_front_matter_and_body() {
        let fm = FrontMatter {
            id: "abc-123".into(),
            tags: vec!["work".into()],
            pinned: true,
            created_at: "2026-01-01T00:00:00Z".into(),
            deleted_at: None,
            is_template: false,
        };
        let body = "Grocery list\nMilk, eggs, bread";
        let raw = serialize(&fm, body);

        let parsed = parse(&raw);
        assert_eq!(parsed.front_matter, Some(fm));
        assert_eq!(parsed.body, body);
    }

    #[test]
    fn plain_file_without_front_matter_has_none() {
        let parsed = parse("Just a note\nwith no front matter");
        assert!(parsed.front_matter.is_none());
        assert_eq!(parsed.body, "Just a note\nwith no front matter");
    }

    #[test]
    fn title_and_preview_split_on_first_line() {
        let body = "Grocery list\nMilk, eggs\nBread";
        assert_eq!(extract_title(body), "Grocery list");
        assert_eq!(extract_preview(body), "Milk, eggs\nBread");
    }

    #[test]
    fn preview_replaces_inlined_images_with_a_placeholder() {
        // Pasted images are inlined as base64 data URIs (see Editor.vue) -
        // a huge one shouldn't leak into the preview/search/graph text.
        let body = format!(
            "Screenshot\nHere: ![a screenshot](data:image/png;base64,{})\nmore text",
            "A".repeat(500_000)
        );
        let preview = extract_preview(&body);
        assert_eq!(preview, "Here: [image]\nmore text");
        assert!(preview.len() < 100);
    }

    #[test]
    fn title_replaces_a_leading_image_with_a_placeholder() {
        let body = "![pasted](data:image/png;base64,AAAA)\nmore text";
        assert_eq!(extract_title(body), "[image]");
    }

    #[test]
    fn preview_leaves_unterminated_image_syntax_alone() {
        // Not well-formed image syntax (no closing bracket/paren) - copied
        // through as plain text rather than silently dropped.
        let body = "Title\n![alt text with no closing paren(oops";
        assert_eq!(
            extract_preview(body),
            "![alt text with no closing paren(oops"
        );
    }

    #[test]
    fn title_strips_markdown_block_markers() {
        assert_eq!(extract_title("## Groceries\nmore"), "Groceries");
        assert_eq!(extract_title("- [ ] Buy milk\nmore"), "Buy milk");
        assert_eq!(extract_title("- [x] Done thing\nmore"), "Done thing");
        assert_eq!(extract_title("- Bullet title\nmore"), "Bullet title");
        assert_eq!(extract_title("1. Ordered title\nmore"), "Ordered title");
        assert_eq!(extract_title("> Quoted title\nmore"), "Quoted title");
        assert_eq!(extract_title("Plain title\nmore"), "Plain title");
    }

    #[test]
    fn title_and_preview_strip_hard_break_markers() {
        // Markdown serializers (ours included) write a trailing '\' to mark
        // a hard line break; it's a rendering artifact, not content.
        let body = "Grocery list\\\nMilk, eggs\\\nBread";
        assert_eq!(extract_title(body), "Grocery list");
        assert_eq!(extract_preview(body), "Milk, eggs\nBread");
    }
}
