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

#[derive(Debug, Clone, PartialEq, Eq)]
enum SourceFingerprint {
    Missing,
    Present {
        len: u64,
        modified: std::time::SystemTime,
    },
}

#[derive(Debug)]
pub(crate) enum SourcePoll {
    Unchanged,
    Loaded(AnnotationSnapshot),
    Failed(AnnotationLoadError),
}

#[derive(Debug)]
pub(crate) struct JsonlFileSource {
    path: PathBuf,
    last_seen: Option<SourceFingerprint>,
}

impl JsonlFileSource {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            last_seen: None,
        }
    }

    pub(crate) async fn poll(&mut self) -> SourcePoll {
        let fingerprint = match tokio::fs::metadata(&self.path).await {
            Ok(metadata) => match metadata.modified() {
                Ok(modified) => SourceFingerprint::Present {
                    len: metadata.len(),
                    modified,
                },
                Err(error) => return SourcePoll::Failed(self.io_error(error)),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                SourceFingerprint::Missing
            }
            Err(error) => return SourcePoll::Failed(self.io_error(error)),
        };

        if self.last_seen.as_ref() == Some(&fingerprint) {
            return SourcePoll::Unchanged;
        }
        self.last_seen = Some(fingerprint.clone());

        match fingerprint {
            SourceFingerprint::Missing => SourcePoll::Failed(AnnotationLoadError {
                path: self.path.clone(),
                line: None,
                reason: "annotation file does not exist".to_string(),
            }),
            SourceFingerprint::Present { .. } => {
                match tokio::fs::read_to_string(&self.path).await {
                    Ok(input) => match parse_jsonl(&self.path, &input) {
                        Ok(snapshot) => SourcePoll::Loaded(snapshot),
                        Err(error) => SourcePoll::Failed(error),
                    },
                    Err(error) => SourcePoll::Failed(self.io_error(error)),
                }
            }
        }
    }

    fn io_error(&self, error: std::io::Error) -> AnnotationLoadError {
        AnnotationLoadError {
            path: self.path.clone(),
            line: None,
            reason: format!("could not read annotation file: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{JsonlFileSource, SourcePoll, parse_jsonl};

    fn temp_path(name: &str) -> PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "grafatui-annotations-{name}-{}-{suffix}.jsonl",
            std::process::id()
        ))
    }

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

    #[tokio::test]
    async fn loads_only_after_metadata_changes() {
        let path = temp_path("changed");
        tokio::fs::write(
            &path,
            "{\"time\":\"2026-07-23T14:30:00Z\",\"text\":\"deploy\"}\n",
        )
        .await
        .unwrap();
        let mut source = JsonlFileSource::new(path.clone());

        assert!(matches!(source.poll().await, SourcePoll::Loaded(snapshot) if snapshot.len() == 1));
        assert!(matches!(source.poll().await, SourcePoll::Unchanged));

        tokio::fs::write(&path, "").await.unwrap();
        assert!(matches!(source.poll().await, SourcePoll::Loaded(snapshot) if snapshot.len() == 0));

        tokio::fs::remove_file(path).await.unwrap();
    }

    #[tokio::test]
    async fn reports_missing_then_loads_when_file_appears() {
        let path = temp_path("appears");
        let mut source = JsonlFileSource::new(path.clone());

        assert!(matches!(source.poll().await, SourcePoll::Failed(_)));
        assert!(matches!(source.poll().await, SourcePoll::Unchanged));

        tokio::fs::write(
            &path,
            "{\"time\":\"2026-07-23T14:30:00Z\",\"text\":\"deploy\"}\n",
        )
        .await
        .unwrap();
        assert!(matches!(source.poll().await, SourcePoll::Loaded(snapshot) if snapshot.len() == 1));

        tokio::fs::remove_file(path).await.unwrap();
    }

    #[tokio::test]
    async fn invalid_replacement_is_reported_as_failed() {
        let path = temp_path("invalid");
        tokio::fs::write(
            &path,
            "{\"time\":\"2026-07-23T14:30:00Z\",\"text\":\"deploy\"}\n",
        )
        .await
        .unwrap();
        let mut source = JsonlFileSource::new(path.clone());
        assert!(matches!(source.poll().await, SourcePoll::Loaded(_)));

        tokio::fs::write(&path, "{\"time\":").await.unwrap();
        assert!(matches!(source.poll().await, SourcePoll::Failed(_)));

        tokio::fs::remove_file(path).await.unwrap();
    }
}
