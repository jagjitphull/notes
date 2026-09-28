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

/// The first non-empty line of the body, used as both the note's display
/// title and the basis for its filename.
pub fn extract_title(body: &str) -> String {
    body.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}

/// The remaining lines after the title, used for the list-view preview.
pub fn extract_preview(body: &str) -> String {
    let mut lines = body.lines().skip_while(|l| l.trim().is_empty());
    lines.next(); // skip the title line itself
    lines.collect::<Vec<_>>().join("\n").trim().to_string()
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
}
