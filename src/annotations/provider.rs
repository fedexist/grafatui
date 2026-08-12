use serde::{Deserialize, Deserializer};
use std::path::PathBuf;
use std::time::Duration;

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
