mod app;
mod grafana;
mod prom;
mod ui;

use std::collections::HashMap;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode},
};
use ratatui::Terminal;
use tokio::time::Duration;

/// Command-line arguments for Grafatui.
#[derive(Debug, Parser, Clone)]
#[command(
    name = "grafatui",
    version,
    about = "Grafana-like Prometheus charts in your terminal"
)]
struct Args {
    /// Prometheus base URL (e.g., "http://localhost:9090")
    #[arg(long, default_value = "http://localhost:9090")]
    prometheus: String,

    /// Time range window (e.g., 5m, 1h, 24h)
    #[arg(long, default_value = "5m")]
    range: String,

    /// Query step resolution (e.g., 5s, 30s, 1m)
    #[arg(long, default_value = "5s")]
    step: String,

    /// Optional Grafana dashboard JSON path; panels with PromQL targets will be imported
    #[arg(long)]
    grafana_json: Option<std::path::PathBuf>,

    /// UI tick rate in milliseconds (screen refresh cadence)
    #[arg(long, default_value = "250")]
    tick_rate: u64,

    /// Data refresh rate in milliseconds (Prometheus fetch interval)
    #[arg(long, default_value = "1000")]
    refresh_rate: u64,

    /// Additional PromQL queries to append as panels
    #[arg(long)]
    query: Vec<String>,

    /// Template variables to override (format: key=value)
    #[arg(long, value_parser = parse_key_val::<String, String>)]
    var: Vec<(String, String)>,
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
    let range = app::parse_duration(&args.range).context("--range")?;
    let step = app::parse_duration(&args.step).context("--step")?;
    let refresh_every = Duration::from_millis(args.refresh_rate);

    let mut vars: HashMap<String, String> = HashMap::new();

    let prom = prom::PromClient::new(args.prometheus.clone());

    // Build panels from Grafana import or simple queries.
    let (title, panels, skipped_panels) = if let Some(path) = args.grafana_json.as_ref() {
        match grafana::load_grafana_dashboard(path) {
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

    let mut state = app::AppState::new(
        prom,
        range,
        step,
        refresh_every,
        title,
        panels,
        skipped_panels,
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
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
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
