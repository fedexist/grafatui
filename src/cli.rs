use clap::Parser;
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
