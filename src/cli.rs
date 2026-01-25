use clap::Parser;
use std::path::PathBuf;

/// Command-line arguments for Grafatui.
#[derive(Debug, Parser, Clone)]
#[command(
    name = "grafatui",
    version,
    about = "Grafana-like Prometheus charts in your terminal"
)]
pub struct Args {
    /// Prometheus URL (e.g., http://localhost:9090)
    #[arg(long)]
    pub prometheus_url: Option<String>,

    /// Time range to query (e.g., 5m, 1h, 3d) (default: 5m)
    #[arg(long, value_name = "DURATION")]
    pub range: Option<String>,

    /// Query step resolution (e.g., 5s, 30s, 1m) (default: 5s)
    #[arg(long, value_name = "DURATION")]
    pub step: Option<String>,

    /// Grafana dashboard JSON file to import (e.g., ./dashboard.json)
    #[arg(long, value_name = "FILE")]
    pub grafana_json: Option<PathBuf>,

    /// UI tick rate in milliseconds (screen refresh cadence)
    #[arg(long, default_value = "250")]
    pub tick_rate: u64,

    /// Data refresh rate in milliseconds (Prometheus fetch interval) (default: 1000)
    #[arg(long, value_name = "MS")]
    pub refresh_rate: Option<u64>,

    /// Additional PromQL queries to append as panels
    #[arg(long, value_name = "EXPR")]
    pub query: Vec<String>,

    /// Template variables to override (e.g., --var instance=server1)
    #[arg(long, value_parser = parse_key_val::<String, String>, value_name = "KEY=VALUE")]
    pub var: Vec<(String, String)>,

    /// Color theme (default, dracula, monokai, solarized-dark, solarized-light, gruvbox, tokyo-night, catppuccin)
    #[arg(long, value_name = "NAME")]
    pub theme: Option<String>,

    /// Configuration file path (e.g., ./grafatui.toml).
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, clap::Subcommand, Clone)]
pub enum Commands {
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
    },
    /// Generate man page
    Man,
}

/// Helper to parse key=value pairs for CLI arguments.
pub fn parse_key_val<T, U>(
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
