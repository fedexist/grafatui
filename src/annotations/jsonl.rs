use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{
    AnnotationEvent, AnnotationProvider, AnnotationProviderError, AnnotationRefreshContext,
    AnnotationSnapshot, AnnotationTarget, ProviderFuture, ProviderPoll,
};

#[derive(Deserialize)]
struct RawAnnotationEvent {
    time: String,
    text: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    panel_titles: RawPanelTitles,
}

#[derive(Default, Deserialize)]
#[serde(untagged)]
enum RawPanelTitles {
    Null(()),
    Titles(Vec<String>),
    #[default]
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnnotationLoadError {
    source: String,
    line: Option<usize>,
    reason: String,
}

impl AnnotationLoadError {
    #[cfg(test)]
    pub(crate) fn line(&self) -> Option<usize> {
        self.line
    }

    fn at_line(source: &str, line: usize, reason: impl Into<String>) -> Self {
        Self {
            source: source.to_string(),
            line: Some(line),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for AnnotationLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(formatter, "{}:{line}: {}", self.source, self.reason),
            None => write!(formatter, "{}: {}", self.source, self.reason),
        }
    }
}

impl std::error::Error for AnnotationLoadError {}

pub(crate) fn parse_jsonl(
    source: &str,
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
                source,
                line_number,
                format!(
                    "invalid annotation event (time, text, tags, and panel_titles must have valid types): {error}"
                ),
            )
        })?;
        let time = DateTime::parse_from_rfc3339(&raw.time)
            .map_err(|error| {
                AnnotationLoadError::at_line(
                    source,
                    line_number,
                    format!("time must be an RFC 3339 timestamp: {error}"),
                )
            })?
            .with_timezone(&Utc);

        if raw.text.trim().is_empty() {
            return Err(AnnotationLoadError::at_line(
                source,
                line_number,
                "text must not be empty",
            ));
        }
        if raw.tags.iter().any(|tag| tag.trim().is_empty()) {
            return Err(AnnotationLoadError::at_line(
                source,
                line_number,
                "tags must not contain empty strings",
            ));
        }

        let target = match raw.panel_titles {
            RawPanelTitles::Missing => AnnotationTarget::All,
            RawPanelTitles::Null(()) => {
                return Err(AnnotationLoadError::at_line(
                    source,
                    line_number,
                    "panel_titles must not be null",
                ));
            }
            RawPanelTitles::Titles(titles) if titles.is_empty() => {
                return Err(AnnotationLoadError::at_line(
                    source,
                    line_number,
                    "panel_titles must contain at least one title",
                ));
            }
            RawPanelTitles::Titles(titles)
                if titles.iter().any(|title| title.trim().is_empty()) =>
            {
                return Err(AnnotationLoadError::at_line(
                    source,
                    line_number,
                    "panel_titles must not contain blank strings",
                ));
            }
            RawPanelTitles::Titles(titles) => {
                AnnotationTarget::PanelTitles(titles.into_iter().collect())
            }
        };

        events.push(AnnotationEvent {
            time,
            text: raw.text,
            tags: raw.tags,
            target,
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
pub(crate) struct JsonlFileProvider {
    path: PathBuf,
    last_seen: Option<SourceFingerprint>,
}

impl JsonlFileProvider {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            last_seen: None,
        }
    }

    async fn poll_file(&mut self) -> ProviderPoll {
        let fingerprint = match tokio::fs::metadata(&self.path).await {
            Ok(metadata) => match metadata.modified() {
                Ok(modified) => SourceFingerprint::Present {
                    len: metadata.len(),
                    modified,
                },
                Err(error) => return ProviderPoll::Failed(self.io_error(error)),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                SourceFingerprint::Missing
            }
            Err(error) => return ProviderPoll::Failed(self.io_error(error)),
        };

        if self.last_seen.as_ref() == Some(&fingerprint) {
            return ProviderPoll::Unchanged;
        }

        match fingerprint {
            SourceFingerprint::Missing => {
                self.last_seen = Some(SourceFingerprint::Missing);
                ProviderPoll::Failed(AnnotationProviderError::new(
                    AnnotationLoadError {
                        source: self.path.display().to_string(),
                        line: None,
                        reason: "annotation file does not exist".to_string(),
                    }
                    .to_string(),
                ))
            }
            SourceFingerprint::Present { len, modified } => {
                match tokio::fs::read_to_string(&self.path).await {
                    Ok(input) => {
                        self.last_seen = Some(SourceFingerprint::Present { len, modified });
                        match parse_jsonl(&self.path.display().to_string(), &input) {
                            Ok(snapshot) => ProviderPoll::Loaded(snapshot),
                            Err(error) => ProviderPoll::Failed(AnnotationProviderError::new(
                                error.to_string(),
                            )),
                        }
                    }
                    Err(error) => ProviderPoll::Failed(self.io_error(error)),
                }
            }
        }
    }

    fn io_error(&self, error: std::io::Error) -> AnnotationProviderError {
        AnnotationProviderError::new(
            AnnotationLoadError {
                source: self.path.display().to_string(),
                line: None,
                reason: format!("could not read annotation file: {error}"),
            }
            .to_string(),
        )
    }
}

impl AnnotationProvider for JsonlFileProvider {
    fn refresh<'a>(&'a mut self, _context: &'a AnnotationRefreshContext) -> ProviderFuture<'a> {
        Box::pin(async move { self.poll_file().await })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use crate::annotations::{
        AnnotationProvider, AnnotationRefreshContext, AnnotationTarget, ProviderPoll,
    };

    use super::{JsonlFileProvider, parse_jsonl};

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

    fn refresh_context() -> AnnotationRefreshContext {
        AnnotationRefreshContext::from_unix_window(200, Duration::from_secs(100))
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

        let snapshot = parse_jsonl("events.jsonl", input).unwrap();

        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot.events()[0].text, "deploy");
        assert_eq!(snapshot.events()[0].tags, vec!["prod"]);
        assert_eq!(snapshot.events()[1].tags, Vec::<String>::new());
    }

    #[test]
    fn parses_optional_panel_titles_and_deduplicates_them() {
        let snapshot = parse_jsonl(
            "events.jsonl",
            concat!(
                r#"{"time":"2026-08-11T14:30:00Z","text":"global"}"#,
                "\n",
                r#"{"time":"2026-08-11T14:31:00Z","text":"targeted","panel_titles":["CPU","CPU","Errors"]}"#,
            ),
        )
        .unwrap();

        assert_eq!(snapshot.events()[0].target, AnnotationTarget::All);
        assert_eq!(
            snapshot.events()[1].target,
            AnnotationTarget::PanelTitles(
                ["CPU".to_string(), "Errors".to_string()]
                    .into_iter()
                    .collect()
            )
        );
    }

    #[test]
    fn rejects_empty_or_blank_panel_titles_with_line_numbers() {
        for input in [
            r#"{"time":"2026-08-11T14:30:00Z","text":"x","panel_titles":[]}"#,
            r#"{"time":"2026-08-11T14:30:00Z","text":"x","panel_titles":["  "]}"#,
        ] {
            let error = parse_jsonl("events.jsonl", input).unwrap_err();
            assert_eq!(error.line(), Some(1));
            assert!(error.to_string().contains("panel_titles"));
        }
    }

    #[test]
    fn rejects_null_panel_titles_with_line_number() {
        let error = parse_jsonl(
            "events.jsonl",
            r#"{"time":"2026-08-11T14:30:00Z","text":"x","panel_titles":null}"#,
        )
        .unwrap_err();

        assert_eq!(error.line(), Some(1));
        assert!(error.to_string().contains("panel_titles"));
    }

    #[test]
    fn rejects_invalid_required_fields_with_line_number() {
        let error = parse_jsonl(
            "events.jsonl",
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
            "events.jsonl",
            r#"{"time":"2026-07-23T14:30:00Z","text":"  "}"#,
        )
        .unwrap_err();
        assert!(blank_text.to_string().contains("text must not be empty"));

        let blank_tag = parse_jsonl(
            "events.jsonl",
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
        let mut provider = JsonlFileProvider::new(path.clone());
        let context = refresh_context();

        assert!(
            matches!(provider.refresh(&context).await, ProviderPoll::Loaded(snapshot) if snapshot.len() == 1)
        );
        assert!(matches!(
            provider.refresh(&context).await,
            ProviderPoll::Unchanged
        ));

        tokio::fs::write(&path, "").await.unwrap();
        assert!(
            matches!(provider.refresh(&context).await, ProviderPoll::Loaded(snapshot) if snapshot.len() == 0)
        );

        tokio::fs::remove_file(path).await.unwrap();
    }

    #[tokio::test]
    async fn reports_missing_then_loads_when_file_appears() {
        let path = temp_path("appears");
        let mut provider = JsonlFileProvider::new(path.clone());
        let context = refresh_context();

        assert!(matches!(
            provider.refresh(&context).await,
            ProviderPoll::Failed(_)
        ));
        assert!(matches!(
            provider.refresh(&context).await,
            ProviderPoll::Unchanged
        ));

        tokio::fs::write(
            &path,
            "{\"time\":\"2026-07-23T14:30:00Z\",\"text\":\"deploy\"}\n",
        )
        .await
        .unwrap();
        assert!(
            matches!(provider.refresh(&context).await, ProviderPoll::Loaded(snapshot) if snapshot.len() == 1)
        );

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
        let mut provider = JsonlFileProvider::new(path.clone());
        let context = refresh_context();
        assert!(matches!(
            provider.refresh(&context).await,
            ProviderPoll::Loaded(_)
        ));

        tokio::fs::write(&path, "{\"time\":").await.unwrap();
        assert!(matches!(
            provider.refresh(&context).await,
            ProviderPoll::Failed(_)
        ));
        assert!(matches!(
            provider.refresh(&context).await,
            ProviderPoll::Unchanged
        ));

        tokio::fs::remove_file(path).await.unwrap();
    }

    #[tokio::test]
    async fn retries_read_failure_when_metadata_fingerprint_is_unchanged() {
        let path = temp_path("read-retry");
        let valid = b"{\"time\":\"2026-07-23T14:30:00Z\",\"text\":\"deploy\"}\n";
        let mut invalid_utf8 = valid.to_vec();
        let text_start = invalid_utf8
            .windows(b"deploy".len())
            .position(|window| window == b"deploy")
            .unwrap();
        invalid_utf8[text_start] = 0xff;
        tokio::fs::write(&path, invalid_utf8).await.unwrap();
        let initial_metadata = tokio::fs::metadata(&path).await.unwrap();
        let initial_modified = initial_metadata.modified().unwrap();
        let mut provider = JsonlFileProvider::new(path.clone());
        let context = refresh_context();

        assert!(
            matches!(provider.refresh(&context).await, ProviderPoll::Failed(error) if error.to_string().contains("could not read annotation file"))
        );

        tokio::fs::write(&path, valid).await.unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(initial_modified)
            .unwrap();
        let recovered_metadata = tokio::fs::metadata(&path).await.unwrap();
        assert_eq!(recovered_metadata.len(), initial_metadata.len());
        assert_eq!(recovered_metadata.modified().unwrap(), initial_modified);

        assert!(matches!(
            provider.refresh(&context).await,
            ProviderPoll::Loaded(snapshot) if snapshot.len() == 1
        ));

        tokio::fs::remove_file(path).await.unwrap();
    }
}
