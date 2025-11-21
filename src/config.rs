use anyhow::Result;
use directories::ProjectDirs;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Config {
    pub prometheus_url: Option<String>,
    pub refresh_rate: Option<u64>,
    pub time_range: Option<String>,
    pub theme: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path();
        if let Some(path) = config_path {
            if path.exists() {
                let content = fs::read_to_string(path)?;
                let config: Config = toml::from_str(&content)?;
                return Ok(config);
            }
        }
        Ok(Config::default())
    }

    fn get_config_path() -> Option<PathBuf> {
        // 1. Check XDG config path
        if let Some(proj_dirs) = ProjectDirs::from("com", "grafatui", "grafatui") {
            let path = proj_dirs.config_dir().join("config.toml");
            if path.exists() {
                return Some(path);
            }
            // Also check for grafatui.toml in config dir
            let path = proj_dirs.config_dir().join("grafatui.toml");
            if path.exists() {
                return Some(path);
            }
        }

        // 2. Check current directory
        let path = PathBuf::from("grafatui.toml");
        if path.exists() {
            return Some(path);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_deserialization() {
        let toml_str = r#"
            prometheus_url = "http://localhost:9090"
            refresh_rate = 5000
            theme = "dracula"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.prometheus_url,
            Some("http://localhost:9090".to_string())
        );
        assert_eq!(config.refresh_rate, Some(5000));
        assert_eq!(config.theme, Some("dracula".to_string()));
    }
}
