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

#[derive(Debug, Parser, Clone)]
#[command(
    name = "grafatui",
    version,
    about = "Grafana-like Prometheus charts in your terminal"
)]
struct Args {
    /// Prometheus base URL
    #[arg(long, default_value = "http://localhost:9090")]
    prometheus: String,

    /// Time range window (e.g., 5m, 1h, 24h)
    #[arg(long, default_value = "5m")]
    range: String,

    /// Query step (e.g., 5s, 30s, 1m)
    #[arg(long, default_value = "5s")]
    step: String,

    /// Optional Grafana dashboard JSON path; panels with PromQL targets will be imported
    #[arg(long)]
    grafana_json: Option<std::path::PathBuf>,

    /// UI tick in milliseconds (screen refresh cadence)
    #[arg(long, default_value_t = 200u64)]
    tick_ms: u64,

    /// Data refresh cadence (how often to re-pull Prometheus data)
    #[arg(long, default_value = "5s")]
    refresh_every: String,

    /// Additional PromQLs when not using Grafana import (repeatable)
    #[arg(long)]
    query: Vec<String>,

    /// Template variables: repeatable KEY=VALUE (e.g., --var model_name=/path/to/model)
    #[arg(long = "var")]
    vars: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let range = app::parse_duration(&args.range).context("--range")?;
    let step = app::parse_duration(&args.step).context("--step")?;
    let refresh_every = app::parse_duration(&args.refresh_every).context("--refresh-every")?;

    let mut vars: HashMap<String, String> = HashMap::new();
    for kv in &args.vars {
        if let Some((k, v)) = kv.split_once('=') {
            vars.insert(k.to_string(), v.to_string());
        }
    }

    let prom = prom::PromClient::new(args.prometheus.clone());

    // Build panels from Grafana import or simple queries.
    let (title, panels) = if let Some(path) = args.grafana_json.as_ref() {
        match grafana::load_grafana_dashboard(path) {
            Ok(d) => {
                let ps = d
                    .queries
                    .into_iter()
                    .map(|q| app::PanelState {
                        title: q.title,
                        exprs: q.exprs,
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
                    })
                    .collect();
                (format!("{} (imported)", d.title), ps)
            }
            Err(e) => {
                eprintln!("Failed to import Grafana dashboard: {e}");
                ("grafatui".to_string(), app::default_queries(args.query))
            }
        }
    } else {
        ("grafatui".to_string(), app::default_queries(args.query))
    };

    let mut state = app::AppState::new(prom, range, step, refresh_every, title, panels);
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
        Duration::from_millis(args.tick_ms),
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
