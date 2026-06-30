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

mod app;
mod config;
mod export;
mod grafana;
mod prom;
mod theme;
mod ui;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use config::Config;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use serde::Serialize;
use theme::Theme;

mod cli;

use cli::Args;

/// Main entry point for the Grafatui application.
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if let Some(cmd) = args.command {
        match cmd {
            cli::Commands::Completions { shell } => {
                use clap::CommandFactory;
                clap_complete::generate(
                    shell,
                    &mut Args::command(),
                    "grafatui",
                    &mut std::io::stdout(),
                );
            }
            cli::Commands::Man => {
                use clap::CommandFactory;
                let man = clap_mangen::Man::new(Args::command());
                man.render(&mut std::io::stdout())?;
            }
        }
        return Ok(());
    }

    // Load config
    let config = match args.config.clone() {
        Some(path) => Config::load(Some(path))?,
        None => Config::load(None).unwrap_or_default(),
    };
    let dashboard_path = args
        .grafana_json
        .clone()
        .or_else(|| config.grafana_json.clone())
        .map(|p| config::expand_path(&p));

    if args.validate {
        let path = dashboard_path.ok_or_else(|| {
            anyhow!("--validate requires --grafana-json or grafana_json in config")
        })?;
        let dashboard = grafana::load_grafana_dashboard(&path)?;
        let summary = validate_dashboard_import(dashboard, config.vars.clone(), &args.var);
        print_validation_summary(&summary, args.format, args.strict)?;
        return Ok(());
    }

    let prometheus_url = args
        .prometheus_url
        .or(config.prometheus_url)
        .unwrap_or_else(|| "http://localhost:9090".to_string());

    let range_str = args
        .range
        .or(config.time_range)
        .unwrap_or_else(|| "5m".to_string());
    let range = app::parse_duration(&range_str).context("--range")?;

    let step_str = args
        .step
        .or(config.step)
        .unwrap_or_else(|| "5s".to_string());
    let step = app::parse_duration(&step_str).context("--step")?;

    let export_dir = args
        .export_dir
        .or(config.export_dir)
        .map(|p| config::expand_path(&p))
        .unwrap_or_else(|| std::path::PathBuf::from("./grafatui-exports"));
    let export_format = args
        .export_format
        .or(config.export_format)
        .unwrap_or_default();
    let record_max_frames = args
        .record_max_frames
        .or(config.record_max_frames)
        .unwrap_or(300);
    let autogrid_enabled = config.autogrid.unwrap_or(true);
    let autogrid_color = args
        .autogrid_color
        .or(config.autogrid_color)
        .map(|color| theme::parse_grafana_color(&color))
        .filter(|color| *color != ratatui::style::Color::Reset)
        .unwrap_or(ratatui::style::Color::DarkGray);

    let mut vars: HashMap<String, String> = HashMap::new();
    let mut query_vars = Vec::new();
    let mut dashboard_refresh_rate_ms = None;

    let prom = prom::PromClient::new(prometheus_url);

    // Build panels from Grafana import or simple queries.
    let (title, panels, skipped_panels) = if let Some(path) = dashboard_path {
        let d = grafana::load_grafana_dashboard(&path)?;
        let import_context = build_import_context(&d, config.vars.clone(), &args.var);
        print_import_diagnostics(&import_context.diagnostics);
        dashboard_refresh_rate_ms = d.refresh_rate_ms;
        vars = import_context.vars;
        query_vars = import_context.query_vars;

        let ps = d
            .queries
            .into_iter()
            .map(|q| app::PanelState {
                title: q.title,
                exprs: q.exprs,
                legends: q.legends,
                query_modes: q.query_modes,
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
                thresholds: q.thresholds,
                min: q.min,
                max: q.max,
                autogrid: q.autogrid,
                display: q.display,
                options: q.options,
            })
            .collect();
        (format!("{} (imported)", d.title), ps, d.skipped_panels)
    } else {
        merge_user_vars(&mut vars, config.vars.clone(), &args.var);
        ("grafatui".to_string(), app::default_queries(args.query), 0)
    };

    // Determine theme
    let theme_name = args
        .theme
        .or(config.theme)
        .unwrap_or_else(|| "default".to_string());
    let theme = Theme::from_str(&theme_name);

    // Determine threshold marker
    let marker_name = args
        .threshold_marker
        .or(config.threshold_marker)
        .unwrap_or_else(|| "dashed-line".to_string());
    let refresh_rate = resolve_refresh_rate_ms(
        args.refresh_rate,
        config.refresh_rate,
        dashboard_refresh_rate_ms,
    );
    let refresh_every = Duration::from_millis(refresh_rate);

    let mut state = app::AppState::new(
        prom,
        range,
        step,
        refresh_every,
        title,
        panels,
        skipped_panels,
        theme,
        marker_name,
        export::ExportOptions {
            dir: export_dir,
            format: export_format,
            record_max_frames,
        }
        .validate()?,
    );
    state.autogrid_enabled = autogrid_enabled;
    state.autogrid_color = autogrid_color;
    state.vars = vars; // <— pass variables into the app
    state.query_vars = query_vars;
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

fn resolve_refresh_rate_ms(
    cli_refresh_rate: Option<u64>,
    config_refresh_rate: Option<u64>,
    dashboard_refresh_rate: Option<u64>,
) -> u64 {
    cli_refresh_rate
        .or(config_refresh_rate)
        .or(dashboard_refresh_rate)
        .unwrap_or(1000)
}

#[derive(Debug)]
struct ImportContext {
    vars: HashMap<String, String>,
    query_vars: Vec<grafana::TemplateQueryVar>,
    diagnostics: Vec<grafana::ImportDiagnostic>,
}

#[derive(Debug, Serialize)]
struct ImportValidationSummary {
    title: String,
    panel_count: usize,
    diagnostics: Vec<grafana::ImportDiagnostic>,
}

fn validate_dashboard_import(
    dashboard: grafana::DashboardImport,
    config_vars: Option<HashMap<String, String>>,
    cli_vars: &[(String, String)],
) -> ImportValidationSummary {
    let import_context = build_import_context(&dashboard, config_vars, cli_vars);
    ImportValidationSummary {
        title: dashboard.title,
        panel_count: dashboard.queries.len(),
        diagnostics: import_context.diagnostics,
    }
}

fn build_import_context(
    dashboard: &grafana::DashboardImport,
    config_vars: Option<HashMap<String, String>>,
    cli_vars: &[(String, String)],
) -> ImportContext {
    let mut vars = dashboard.vars.clone();
    let pinned_vars = merge_user_vars(&mut vars, config_vars, cli_vars);

    let query_vars = dashboard
        .query_vars
        .iter()
        .filter(|var| !pinned_vars.contains(&var.name))
        .cloned()
        .collect();
    let mut diagnostics = dashboard.diagnostics.clone();
    diagnostics.extend(grafana::variable_diagnostics(dashboard, &vars));

    ImportContext {
        vars,
        query_vars,
        diagnostics,
    }
}

fn merge_user_vars(
    vars: &mut HashMap<String, String>,
    config_vars: Option<HashMap<String, String>>,
    cli_vars: &[(String, String)],
) -> HashSet<String> {
    let mut pinned_vars = HashSet::new();
    if let Some(config_vars) = config_vars {
        for (k, v) in config_vars {
            pinned_vars.insert(k.clone());
            vars.insert(k, v);
        }
    }

    for (k, v) in cli_vars {
        pinned_vars.insert(k.clone());
        vars.insert(k.clone(), v.clone());
    }

    pinned_vars
}

fn print_import_diagnostics(diagnostics: &[grafana::ImportDiagnostic]) {
    if diagnostics.is_empty() {
        return;
    }

    eprintln!(
        "Grafana import diagnostics: {} warning(s)",
        diagnostics.len()
    );
    for diagnostic in diagnostics {
        eprintln!(
            "warning[grafana.import.{}] {}: {}",
            diagnostic.code, diagnostic.path, diagnostic.message
        );
    }
}

fn print_validation_summary(
    summary: &ImportValidationSummary,
    format: cli::ValidateFormat,
    strict: bool,
) -> Result<()> {
    match format {
        cli::ValidateFormat::Text => {
            print_import_diagnostics(&summary.diagnostics);
            if strict && !summary.diagnostics.is_empty() {
                bail!(
                    "validation failed with {} warning(s)",
                    summary.diagnostics.len()
                );
            }
            println!(
                "Grafana dashboard is importable: {} ({} panel(s))",
                summary.title, summary.panel_count
            );
        }
        cli::ValidateFormat::Json => {
            println!("{}", serde_json::to_string_pretty(summary)?);
            if strict && !summary.diagnostics.is_empty() {
                bail!(
                    "validation failed with {} warning(s)",
                    summary.diagnostics.len()
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_refresh_rate_precedence() {
        assert_eq!(
            resolve_refresh_rate_ms(Some(2000), Some(3000), Some(4000)),
            2000
        );
        assert_eq!(resolve_refresh_rate_ms(None, Some(3000), Some(4000)), 3000);
        assert_eq!(resolve_refresh_rate_ms(None, None, Some(4000)), 4000);
        assert_eq!(resolve_refresh_rate_ms(None, None, None), 1000);
    }

    #[test]
    fn test_validate_dashboard_import_adds_variable_diagnostics_without_prometheus() {
        let json = r#"{
            "title": "Validate",
            "panels": [
                {
                    "type": "timeseries",
                    "title": "CPU",
                    "targets": [
                        { "expr": "up{job=\"$job\", cluster=\"$cluster\"}" }
                    ]
                }
            ]
        }"#;
        let path = std::env::temp_dir().join("grafatui-validate-helper-test.json");
        std::fs::write(&path, json).unwrap();
        let dashboard = grafana::load_grafana_dashboard(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        let summary =
            validate_dashboard_import(dashboard, None, &[("job".to_string(), "node".to_string())]);

        assert_eq!(summary.title, "Validate");
        assert_eq!(summary.panel_count, 1);
        assert_eq!(summary.diagnostics.len(), 1);
        assert_eq!(summary.diagnostics[0].code, "unresolved_variable");
        assert!(summary.diagnostics[0].message.contains("$cluster"));
    }

    #[test]
    fn test_merge_user_vars_applies_config_and_cli_overrides() {
        let mut vars = HashMap::new();
        vars.insert("job".to_string(), "dashboard".to_string());
        let mut config_vars = HashMap::new();
        config_vars.insert("job".to_string(), "config".to_string());
        config_vars.insert("instance".to_string(), "config-instance".to_string());

        let pinned = merge_user_vars(
            &mut vars,
            Some(config_vars),
            &[("job".to_string(), "cli".to_string())],
        );

        assert_eq!(vars.get("job"), Some(&"cli".to_string()));
        assert_eq!(vars.get("instance"), Some(&"config-instance".to_string()));
        assert!(pinned.contains("job"));
        assert!(pinned.contains("instance"));
    }
}
