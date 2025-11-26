mod app;
mod config;
mod grafana;
mod prom;
mod theme;
mod ui;

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use config::Config;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use theme::Theme;

/// Command-line arguments for Grafatui.
#[derive(Debug, Parser, Clone)]
#[command(
    name = "grafatui",
    version,
    about = "Grafana-like Prometheus charts in your terminal"
)]
struct Args {
    /// Prometheus base URL (default: http://localhost:9090)
    #[arg(long)]
    prometheus: Option<String>,

    /// Time range window (e.g., 5m, 1h, 24h) (default: 5m)
    #[arg(long)]
    range: Option<String>,

    /// Query step resolution (e.g., 5s, 30s, 1m) (default: 5s)
    #[arg(long)]
    step: Option<String>,

    /// Optional Grafana dashboard JSON path; panels with PromQL targets will be imported
    #[arg(long)]
    grafana_json: Option<std::path::PathBuf>,

    /// UI tick rate in milliseconds (screen refresh cadence)
    #[arg(long, default_value = "250")]
    tick_rate: u64,

    /// Data refresh rate in milliseconds (Prometheus fetch interval) (default: 1000)
    #[arg(long)]
    refresh_rate: Option<u64>,

    /// Additional PromQL queries to append as panels
    #[arg(long)]
    query: Vec<String>,

    /// Template variables to override (format: key=value)
    #[arg(long, value_parser = parse_key_val::<String, String>)]
    var: Vec<(String, String)>,

    /// Theme name (e.g. "dracula", "monokai") (default: default)
    #[arg(long)]
    theme: Option<String>,

    /// Path to configuration file
    #[arg(long)]
    config: Option<std::path::PathBuf>,
}

/// Helper to parse key=value pairs for CLI arguments.
fn parse_key_val<T, U>(
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

/// Main entry point for the Grafatui application.
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    // Load config
    let config = Config::load(args.config.clone()).unwrap_or_default();

    let prometheus_url = args
        .prometheus
        .or(config.prometheus_url)
        .unwrap_or_else(|| "http://localhost:9090".to_string());

    let range_str = args
        .range
        .or(config.time_range)
        .unwrap_or_else(|| "5m".to_string());
    let range = app::parse_duration(&range_str).context("--range")?;

    let step_str = args.step.unwrap_or_else(|| "5s".to_string());
    let step = app::parse_duration(&step_str).context("--step")?;

    let refresh_rate = args.refresh_rate.or(config.refresh_rate).unwrap_or(1000);
    let refresh_every = Duration::from_millis(refresh_rate);

    let mut vars: HashMap<String, String> = HashMap::new();

    let prom = prom::PromClient::new(prometheus_url);

    // Build panels from Grafana import or simple queries.
    let (title, panels, skipped_panels) =
        if let Some(path) = args.grafana_json.or(config.grafana_json) {
            match grafana::load_grafana_dashboard(&path) {
                Ok(d) => {
                    // Seed vars from dashboard defaults
                    for (k, v) in d.vars {
                        vars.insert(k, v);
                    }

                    let ps = d
                        .queries
                        .into_iter()
                        .map(|q| app::PanelState {
                            title: q.title,
                            exprs: q.exprs,
                            legends: q.legends,
                            series: vec![],
                            last_error: None,
                            last_url: None,
                            last_samples: 0,
                            grid: q.grid.map(|g| app::GridUnit {
                                x: g.x,
                                y: g.y,
                                w: g.w,
                                h: g.h,
                            }),
                            y_axis_mode: app::YAxisMode::Auto,
                            panel_type: q.panel_type,
                        })
                        .collect();
                    (format!("{} (imported)", d.title), ps, d.skipped_panels)
                }
                Err(e) => {
                    eprintln!("Failed to import Grafana dashboard: {e}");
                    ("grafatui".to_string(), app::default_queries(args.query), 0)
                }
            }
        } else {
            ("grafatui".to_string(), app::default_queries(args.query), 0)
        };

    // CLI vars override dashboard defaults
    for (k, v) in &args.var {
        vars.insert(k.clone(), v.clone());
    }

    // Determine theme
    let theme_name = args
        .theme
        .or(config.theme)
        .unwrap_or_else(|| "default".to_string());
    let theme = Theme::from_str(&theme_name);

    let mut state = app::AppState::new(
        prom,
        range,
        step,
        refresh_every,
        title,
        panels,
        skipped_panels,
        theme,
    );
    state.vars = vars; // <— pass variables into the app
    state.refresh().await?;

    // Terminal setup
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = app::run_app(
        &mut terminal,
        &mut state,
        Duration::from_millis(args.tick_rate),
    )
    .await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}
