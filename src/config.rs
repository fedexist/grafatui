/*
 * Copyright 2025 Federico D'Ambrosio
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use anyhow::Result;
use directories::ProjectDirs;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Config {
    pub prometheus_url: Option<String>,
    pub refresh_rate: Option<u64>,
    pub time_range: Option<String>,
    pub step: Option<String>,
    pub theme: Option<String>,
    pub grafana_json: Option<PathBuf>,
    pub vars: Option<HashMap<String, String>>,
}

impl Config {
    pub fn load(cli_path: Option<PathBuf>) -> Result<Self> {
        let config_path = cli_path.or_else(Self::get_config_path);
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
        if let Some(base_dirs) = ProjectDirs::from("", "", "grafatui") {
            let config_path = base_dirs.config_dir().join("config.toml");
            if config_path.exists() {
                return Some(config_path);
            }
            let config_path = base_dirs.config_dir().join("grafatui.toml");
            if config_path.exists() {
                return Some(config_path);
            }
        }

        let cwd_path = PathBuf::from("./grafatui.toml");
        if cwd_path.exists() {
            return Some(cwd_path.to_path_buf());
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
