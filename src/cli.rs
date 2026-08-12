/*
 * Copyright 2026 Federico D'Ambrosio
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

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// Command-line arguments for Grafatui.
#[derive(Debug, Parser, Clone)]
#[command(
    name = "grafatui",
    version,
    about = "Grafana-like Prometheus charts in your terminal"
)]
pub(crate) struct Args {
    /// Prometheus URL (e.g., http://localhost:9090)
    #[arg(long)]
    pub(crate) prometheus_url: Option<String>,

    /// Time range to query (e.g., 5m, 1h, 3d) (default: 5m)
    #[arg(long, value_name = "DURATION")]
    pub(crate) range: Option<String>,

    /// Query step resolution (e.g., 5s, 30s, 1m) (default: 5s)
    #[arg(long, value_name = "DURATION")]
    pub(crate) step: Option<String>,

    /// Grafana dashboard JSON file to import (e.g., ./dashboard.json)
    #[arg(long, value_name = "FILE")]
    pub(crate) grafana_json: Option<PathBuf>,

    /// Optional JSONL point-event file to overlay on graph panels.
    #[arg(
        long,
        value_name = "FILE",
        conflicts_with_all = [
            "annotations_command",
            "annotations_command_arg",
            "annotations_command_timeout"
        ]
    )]
    pub(crate) annotations_file: Option<PathBuf>,

    /// Optional executable annotation provider.
    #[arg(long, value_name = "PROGRAM", conflicts_with = "annotations_file")]
    pub(crate) annotations_command: Option<String>,

    /// Argument passed to --annotations-command; repeat to preserve argument order.
    #[arg(
        long,
        value_name = "ARG",
        requires = "annotations_command",
        allow_hyphen_values = true
    )]
    pub(crate) annotations_command_arg: Vec<String>,

    /// Maximum annotation-command runtime (for example, 500ms or 10s).
    #[arg(long, value_name = "DURATION", requires = "annotations_command")]
    pub(crate) annotations_command_timeout: Option<String>,

    /// Validate a Grafana dashboard import without starting the TUI
    #[arg(long)]
    pub(crate) validate: bool,

    /// Fail validation when import diagnostics contain warnings
    #[arg(long, requires = "validate")]
    pub(crate) strict: bool,

    /// Output format for --validate
    #[arg(long, value_enum, default_value = "text", requires = "validate")]
    pub(crate) format: ValidateFormat,

    /// Legacy UI tick rate in milliseconds; redraws now happen on input and data refresh
    #[arg(long, default_value = "250")]
    pub(crate) tick_rate: u64,

    /// Data refresh rate in milliseconds (Prometheus fetch interval) (default: 1000)
    #[arg(long, value_name = "MS")]
    pub(crate) refresh_rate: Option<u64>,

    /// Additional PromQL queries to append as panels
    #[arg(long, value_name = "EXPR")]
    pub(crate) query: Vec<String>,

    /// Template variables to override (e.g., --var instance=server1)
    #[arg(long, value_parser = parse_key_val::<String, String>, value_name = "KEY=VALUE")]
    pub(crate) var: Vec<(String, String)>,

    /// Color theme (default, dracula, monokai, solarized-dark, solarized-light, gruvbox, tokyo-night, catppuccin)
    #[arg(long, value_name = "NAME")]
    pub(crate) theme: Option<String>,

    /// Marker symbol to use for threshold lines (dashed, dot, braille, block, bar, quadrant, sextant, octant)
    #[arg(long, value_name = "MARKER")]
    pub(crate) threshold_marker: Option<String>,

    /// Color to use for automatic grid lines and labels (e.g., gray, dark-gray, #666666).
    #[arg(long, value_name = "COLOR")]
    pub(crate) autogrid_color: Option<String>,

    /// Directory for SVG/PNG exports and recordings.
    #[arg(long, value_name = "DIR")]
    pub(crate) export_dir: Option<PathBuf>,

    /// Image format to export.
    #[arg(long, value_enum, value_name = "FORMAT")]
    pub(crate) export_format: Option<crate::export::ExportFormat>,

    /// Maximum number of changed frames to keep in one recording; must be greater than zero.
    #[arg(long, value_name = "COUNT")]
    pub(crate) record_max_frames: Option<usize>,

    /// Configuration file path (e.g., ./grafatui.toml).
    #[arg(long, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Option<Commands>,
}

#[derive(Debug, clap::Subcommand, Clone)]
pub(crate) enum Commands {
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
    },
    /// Generate man page
    Man,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum ValidateFormat {
    #[default]
    Text,
    Json,
}

/// Helper to parse key=value pairs for CLI arguments.
pub(crate) fn parse_key_val<T, U>(
    s: &str,
) -> Result<(T, U), Box<dyn std::error::Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: std::error::Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{}`", s))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_parse_validate_with_grafana_json() {
        let args = Args::parse_from(["grafatui", "--validate", "--grafana-json", "dashboard.json"]);

        assert!(args.validate);
        assert_eq!(args.grafana_json, Some(PathBuf::from("dashboard.json")));
    }

    #[test]
    fn test_parse_validate_strict_and_json_format() {
        let args = Args::parse_from([
            "grafatui",
            "--validate",
            "--strict",
            "--format",
            "json",
            "--grafana-json",
            "dashboard.json",
        ]);

        assert!(args.validate);
        assert!(args.strict);
        assert_eq!(args.format, crate::cli::ValidateFormat::Json);
    }

    #[test]
    fn test_parse_annotations_file() {
        let args = Args::parse_from(["grafatui", "--annotations-file", "events.jsonl"]);
        assert_eq!(args.annotations_file, Some(PathBuf::from("events.jsonl")));
    }

    #[test]
    fn parses_annotation_command_with_ordered_hyphen_arguments() {
        let args = Args::try_parse_from([
            "grafatui",
            "--annotations-command",
            "./provider",
            "--annotations-command-arg=--environment",
            "--annotations-command-arg=prod",
            "--annotations-command-timeout",
            "750ms",
        ])
        .unwrap();

        assert_eq!(args.annotations_command.as_deref(), Some("./provider"));
        assert_eq!(args.annotations_command_arg, ["--environment", "prod"]);
        assert_eq!(args.annotations_command_timeout.as_deref(), Some("750ms"));
    }

    #[test]
    fn rejects_partial_or_conflicting_annotation_command_flags() {
        assert!(Args::try_parse_from(["grafatui", "--annotations-command-arg=x"]).is_err());
        assert!(
            Args::try_parse_from([
                "grafatui",
                "--annotations-file",
                "events.jsonl",
                "--annotations-command",
                "./provider"
            ])
            .is_err()
        );
    }
}
