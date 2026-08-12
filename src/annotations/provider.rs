use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use super::{AnnotationSnapshot, command::CommandProvider, jsonl::JsonlFileProvider};

pub(crate) const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct AnnotationCommandConfig {
    pub(crate) program: String,
    #[serde(default)]
    pub(crate) args: Vec<String>,
    #[serde(
        default = "default_command_timeout",
        deserialize_with = "deserialize_duration"
    )]
    pub(crate) timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum AnnotationSourceConfig {
    File(PathBuf),
    Command(AnnotationCommandConfig),
}

pub(crate) type ProviderFuture<'a> = Pin<Box<dyn Future<Output = ProviderPoll> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnnotationRefreshContext {
    pub(crate) from: DateTime<Utc>,
    pub(crate) to: DateTime<Utc>,
}

impl AnnotationRefreshContext {
    pub(crate) fn from_unix_window(end_ts: i64, range: Duration) -> Self {
        let to = DateTime::<Utc>::from_timestamp(end_ts, 0).expect("valid Unix timestamp");
        let from = to - TimeDelta::from_std(range).expect("supported dashboard range");
        Self { from, to }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnnotationProviderError(String);

impl AnnotationProviderError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    #[allow(dead_code)] // Used by command providers before they are runtime-wired.
    pub(crate) fn command(program: &str, reason: impl fmt::Display) -> Self {
        Self(format!("annotation command {program}: {reason}"))
    }
}

impl fmt::Display for AnnotationProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug)]
pub(crate) enum ProviderPoll {
    Unchanged,
    Loaded(AnnotationSnapshot),
    Failed(AnnotationProviderError),
}

pub(crate) trait AnnotationProvider: fmt::Debug + Send {
    fn refresh<'a>(&'a mut self, context: &'a AnnotationRefreshContext) -> ProviderFuture<'a>;
}

pub(crate) fn build_provider(source: AnnotationSourceConfig) -> Box<dyn AnnotationProvider> {
    match source {
        AnnotationSourceConfig::File(path) => Box::new(JsonlFileProvider::new(path)),
        AnnotationSourceConfig::Command(config) => Box::new(CommandProvider::new(config)),
    }
}

fn default_command_timeout() -> Duration {
    DEFAULT_COMMAND_TIMEOUT
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    humantime::parse_duration(&value).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::{
        AnnotationCommandConfig, AnnotationProvider, AnnotationRefreshContext,
        AnnotationSourceConfig, ProviderFuture, ProviderPoll, build_provider,
    };
    use crate::annotations::AnnotationSnapshot;

    #[derive(Debug)]
    struct StaticProvider;

    impl AnnotationProvider for StaticProvider {
        fn refresh<'a>(&'a mut self, _context: &'a AnnotationRefreshContext) -> ProviderFuture<'a> {
            Box::pin(async { ProviderPoll::Loaded(AnnotationSnapshot::new(Vec::new())) })
        }
    }

    #[tokio::test]
    async fn provider_contract_is_object_safe() {
        let mut provider: Box<dyn AnnotationProvider> = Box::new(StaticProvider);
        let context = AnnotationRefreshContext::from_unix_window(100, Duration::from_secs(10));
        assert!(matches!(
            provider.refresh(&context).await,
            ProviderPoll::Loaded(snapshot) if snapshot.len() == 0
        ));
    }

    fn temp_path(name: &str) -> PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "grafatui-provider-{name}-{}-{suffix}.jsonl",
            std::process::id()
        ))
    }

    #[tokio::test]
    async fn builds_file_provider_that_loads_jsonl() {
        let path = temp_path("construction");
        tokio::fs::write(
            &path,
            "{\"time\":\"2026-08-12T10:00:00Z\",\"text\":\"deploy\"}\n",
        )
        .await
        .unwrap();
        let mut provider = build_provider(AnnotationSourceConfig::File(path.clone()));
        let context = AnnotationRefreshContext::from_unix_window(200, Duration::from_secs(10));

        let ProviderPoll::Loaded(snapshot) = provider.refresh(&context).await else {
            panic!("expected file provider to load a snapshot");
        };
        assert_eq!(snapshot.events()[0].text, "deploy");

        tokio::fs::remove_file(path).await.unwrap();
    }

    #[tokio::test]
    async fn builds_command_provider_that_identifies_spawn_failure() {
        let missing_program = std::env::temp_dir()
            .join(format!(
                "grafatui-missing-provider-factory-{}",
                std::process::id()
            ))
            .to_string_lossy()
            .into_owned();
        let mut provider =
            build_provider(AnnotationSourceConfig::Command(AnnotationCommandConfig {
                program: missing_program.clone(),
                args: Vec::new(),
                timeout: Duration::from_secs(1),
            }));
        let context = AnnotationRefreshContext::from_unix_window(200, Duration::from_secs(10));

        let ProviderPoll::Failed(error) = provider.refresh(&context).await else {
            panic!("expected command provider spawn failure");
        };
        assert!(error.to_string().contains(&missing_program));
        assert!(error.to_string().contains("could not start"));
    }
}
