use std::fmt;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{AnnotationEvent, AnnotationSnapshot, AnnotationTarget};

#[derive(Deserialize)]
struct RawAnnotationEvent {
    time: String,
    text: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnnotationLoadError {
    path: PathBuf,
    line: Option<usize>,
    reason: String,
}

impl AnnotationLoadError {
    pub(crate) fn line(&self) -> Option<usize> {
        self.line
    }

    fn at_line(path: &Path, line: usize, reason: impl Into<String>) -> Self {
        Self {
            path: path.to_path_buf(),
            line: Some(line),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for AnnotationLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(formatter, "{}:{line}: {}", self.path.display(), self.reason),
            None => write!(formatter, "{}: {}", self.path.display(), self.reason),
        }
    }
}

impl std::error::Error for AnnotationLoadError {}

pub(crate) fn parse_jsonl(
    path: &Path,
    input: &str,
) -> Result<AnnotationSnapshot, AnnotationLoadError> {
    let mut events = Vec::new();

    for (index, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let line_number = index + 1;
        let raw: RawAnnotationEvent = serde_json::from_str(line).map_err(|error| {
            AnnotationLoadError::at_line(
                path,
                line_number,
                format!(
                    "invalid annotation event (time, text, and tags must have valid types): {error}"
                ),
            )
        })?;
        let time = DateTime::parse_from_rfc3339(&raw.time)
            .map_err(|error| {
                AnnotationLoadError::at_line(
                    path,
                    line_number,
                    format!("time must be an RFC 3339 timestamp: {error}"),
                )
            })?
            .with_timezone(&Utc);

        if raw.text.trim().is_empty() {
            return Err(AnnotationLoadError::at_line(
                path,
                line_number,
                "text must not be empty",
            ));
        }
        if raw.tags.iter().any(|tag| tag.trim().is_empty()) {
            return Err(AnnotationLoadError::at_line(
                path,
                line_number,
                "tags must not contain empty strings",
            ));
        }

        events.push(AnnotationEvent {
            time,
            text: raw.text,
            tags: raw.tags,
            target: AnnotationTarget::All,
        });
    }

    Ok(AnnotationSnapshot::new(events))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::parse_jsonl;

    #[test]
    fn parses_valid_jsonl_and_ignores_blank_lines_and_unknown_fields() {
        let input = concat!(
            "\n",
            r#"{"time":"2026-07-23T14:30:00+02:00","text":"deploy","tags":["prod"],"extra":1}"#,
            "\n",
            r#"{"time":"2026-07-23T13:00:00Z","text":"rollback"}"#,
            "\n",
        );

        let snapshot = parse_jsonl(Path::new("events.jsonl"), input).unwrap();

        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot.events()[0].text, "deploy");
        assert_eq!(snapshot.events()[0].tags, vec!["prod"]);
        assert_eq!(snapshot.events()[1].tags, Vec::<String>::new());
    }

    #[test]
    fn rejects_invalid_required_fields_with_line_number() {
        let error = parse_jsonl(
            Path::new("events.jsonl"),
            "{\"time\":1700000000000,\"text\":\"deploy\"}\n",
        )
        .unwrap_err();

        assert_eq!(error.line(), Some(1));
        assert!(error.to_string().contains("events.jsonl:1"));
        assert!(error.to_string().contains("time"));
    }

    #[test]
    fn rejects_blank_text_and_tags() {
        let blank_text = parse_jsonl(
            Path::new("events.jsonl"),
            r#"{"time":"2026-07-23T14:30:00Z","text":"  "}"#,
        )
        .unwrap_err();
        assert!(blank_text.to_string().contains("text must not be empty"));

        let blank_tag = parse_jsonl(
            Path::new("events.jsonl"),
            r#"{"time":"2026-07-23T14:30:00Z","text":"deploy","tags":[""]}"#,
        )
        .unwrap_err();
        assert!(
            blank_tag
                .to_string()
                .contains("tags must not contain empty strings")
        );
    }
}
