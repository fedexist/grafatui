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

#![forbid(unsafe_code)]

mod annotations;
mod app;
mod config;
mod dashboard;
mod display_units;
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

use annotations::{AnnotationCommandConfig, AnnotationSourceConfig};
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
    let config = load_startup_config(args.config.clone())?;
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

    let annotation_source = resolve_annotation_source(
        AnnotationCliSource {
            file: args.annotations_file,
            program: args.annotations_command,
            args: args.annotations_command_arg,
            timeout: args.annotations_command_timeout,
        },
        config.annotations_file,
        config.annotations_command,
    )?;

    let prometheus_url = args
        .prometheus_url
        .or(config.prometheus_url)
        .unwrap_or_else(|| "http://localhost:9090".to_string());

    let range_str = args
        .range
        .or(config.time_range)
        .unwrap_or_else(|| "5m".to_string());
    let range = app::parse_duration(&range_str).context("--range")?;

    let step_policy =
        app::resolve_step_policy(args.step.as_deref(), config.step.as_deref()).context("--step")?;

    let scrape_interval = match args.scrape_interval.or(config.scrape_interval) {
        Some(text) => app::parse_duration(&text).context("--scrape-interval")?,
        None => app::DEFAULT_SCRAPE_INTERVAL,
    };
    if scrape_interval.is_zero() {
        bail!("--scrape-interval must be greater than zero");
    }

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
    let mut variable_state = dashboard::variables::VariableState::default();
    let mut auto_grid_behaviors = HashMap::new();
    let mut dashboard_refresh_rate_ms = None;

    let prom = prom::PromClient::new(prometheus_url);

    // Build panels from Grafana import or simple queries.
    let (title, panels, skipped_panels, imported_layout) = if let Some(path) = dashboard_path {
        let d = grafana::load_grafana_dashboard(&path)?;
        let import_context = build_import_context(&d, config.vars.clone(), &args.var);
        print_import_diagnostics(&import_context.diagnostics);
        dashboard_refresh_rate_ms = d.refresh_rate_ms;
        vars = import_context.vars;
        query_vars = import_context.query_vars;
        variable_state = d.variable_state;
        auto_grid_behaviors = d.auto_grid_behaviors;
        merge_user_vars(
            &mut variable_state.overrides,
            config.vars.clone(),
            &args.var,
        );

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
                resolution: q.resolution,
            })
            .collect();
        (
            format!("{} (imported)", d.title),
            ps,
            d.skipped_panels,
            Some(d.layout),
        )
    } else {
        merge_user_vars(&mut vars, config.vars.clone(), &args.var);
        (
            "grafatui".to_string(),
            app::default_queries(args.query),
            0,
            None,
        )
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
        step_policy,
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
    apply_imported_layout(&mut state, imported_layout);
    state.annotations = annotations::AnnotationState::from_source(annotation_source);
    state.scrape_interval = scrape_interval;
    state.autogrid_enabled = autogrid_enabled;
    state.autogrid_color = autogrid_color;
    state.vars = vars; // <— pass variables into the app
    state.query_vars = query_vars;
    state.variable_state = variable_state;
    state.configure_dynamic(auto_grid_behaviors);
    // Signals are handled from here on, so stopping during the first refresh,
    // which may be waiting on an annotation provider, still cleans up.
    let mut shutdown = ShutdownSignals::register();
    tokio::select! {
        res = state.refresh_initial() => res?,
        () = shutdown.recv() => return Ok(()),
    }

    install_terminal_panic_hook();
    let guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    let res = tokio::select! {
        res = app::run_app(
            &mut terminal,
            &mut state,
            Duration::from_millis(args.tick_rate),
        ) => res,
        () = shutdown.recv() => Ok(()),
    };
    // Save a recording however the session ended; this is a no-op when the
    // event loop already saved it on quit.
    let finalized = app::finalize_recording_before_quit(&mut state);

    drop(guard);
    res.and(finalized)
}

/// Signals asking Grafatui to stop: SIGINT, and on Unix SIGTERM or SIGHUP (the
/// terminal closing).
///
/// Handlers are installed when this is created, replacing the default action
/// of killing the process outright. Stopping through `recv` instead drops the
/// running refresh, so annotation provider processes are killed and the
/// terminal is restored.
struct ShutdownSignals {
    #[cfg(unix)]
    signals: Vec<tokio::signal::unix::Signal>,
}

impl ShutdownSignals {
    fn register() -> Self {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            Self {
                signals: [
                    SignalKind::interrupt(),
                    SignalKind::terminate(),
                    SignalKind::hangup(),
                ]
                .into_iter()
                .filter_map(|kind| signal(kind).ok())
                .collect(),
            }
        }
        #[cfg(not(unix))]
        {
            Self {}
        }
    }

    /// Resolves when any of the signals arrives. Cancel-safe.
    async fn recv(&mut self) {
        #[cfg(unix)]
        {
            let received = self
                .signals
                .iter_mut()
                .map(|signal| Box::pin(signal.recv()));
            if received.len() == 0 {
                return std::future::pending().await;
            }
            futures::future::select_all(received).await;
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

/// Raw mode, the alternate screen, and mouse capture, undone when dropped,
/// including on early returns and while unwinding from a panic.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        // From here on, dropping the guard restores the terminal.
        let guard = Self;
        execute!(
            std::io::stdout(),
            EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Restores the terminal, continuing past individual failures so one failing
/// step cannot leave the others undone.
fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        std::io::stdout(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
        crossterm::cursor::Show
    );
}

/// Restores the terminal before the default panic message is printed, so the
/// message lands on the normal screen instead of the alternate one.
fn install_terminal_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous(info);
    }));
}

fn apply_imported_layout(
    state: &mut app::AppState,
    imported_layout: Option<dashboard::DashboardLayout>,
) {
    if let Some(layout) = imported_layout {
        state.apply_layout(layout);
    }
}

fn load_startup_config(path: Option<std::path::PathBuf>) -> Result<Config> {
    Config::load(path)
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

#[derive(Debug, Default)]
struct AnnotationCliSource {
    file: Option<std::path::PathBuf>,
    program: Option<String>,
    args: Vec<String>,
    timeout: Option<String>,
}

fn resolve_annotation_source(
    cli: AnnotationCliSource,
    config_file: Option<std::path::PathBuf>,
    config_command: Option<AnnotationCommandConfig>,
) -> Result<Option<AnnotationSourceConfig>> {
    if config_file.is_some() && config_command.is_some() {
        bail!("annotations_file and annotations_command cannot both be configured");
    }
    if cli.file.is_some() && cli.program.is_some() {
        bail!("--annotations-file and --annotations-command cannot be combined");
    }
    if cli.program.is_none() && (!cli.args.is_empty() || cli.timeout.is_some()) {
        bail!("annotation command arguments and timeout require --annotations-command");
    }

    if let Some(path) = cli.file {
        return Ok(Some(AnnotationSourceConfig::File(config::expand_path(
            &path,
        ))));
    }
    if let Some(program) = cli.program {
        let timeout = match cli.timeout {
            Some(value) => humantime::parse_duration(&value)
                .with_context(|| "--annotations-command-timeout")?,
            None => annotations::DEFAULT_COMMAND_TIMEOUT,
        };
        validate_annotation_command(&program, timeout)?;
        return Ok(Some(AnnotationSourceConfig::Command(
            AnnotationCommandConfig {
                program,
                args: cli.args,
                timeout,
            },
        )));
    }

    if let Some(command) = config_command {
        validate_annotation_command(&command.program, command.timeout)?;
        return Ok(Some(AnnotationSourceConfig::Command(command)));
    }
    Ok(config_file.map(|path| AnnotationSourceConfig::File(config::expand_path(&path))))
}

fn validate_annotation_command(program: &str, timeout: Duration) -> Result<()> {
    if program.trim().is_empty() {
        bail!("annotation command program must not be empty");
    }
    if timeout.is_zero() {
        bail!("annotation command timeout must be greater than zero");
    }
    Ok(())
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
    use crate::dashboard::{DashboardLayout, DashboardLayoutItem, DashboardRow, RowId};

    #[test]
    #[ignore = "requires a real PTY"]
    fn terminal_guard_restores_after_application_error() {
        assert!(!crossterm::terminal::is_raw_mode_enabled().unwrap());
        let result: Result<()> = (|| {
            let _guard = TerminalGuard::enter()?;
            anyhow::bail!("controlled application error")
        })();

        assert_eq!(
            result.unwrap_err().to_string(),
            "controlled application error"
        );
        assert!(!crossterm::terminal::is_raw_mode_enabled().unwrap());
    }

    #[test]
    #[ignore = "requires a real PTY"]
    fn terminal_guard_restores_during_unwind() {
        assert!(!crossterm::terminal::is_raw_mode_enabled().unwrap());
        install_terminal_panic_hook();

        let panic = std::panic::catch_unwind(|| {
            let _guard = TerminalGuard::enter().unwrap();
            panic!("controlled terminal panic");
        });

        assert!(panic.is_err());
        assert!(!crossterm::terminal::is_raw_mode_enabled().unwrap());
    }

    fn temp_config_path(name: &str) -> std::path::PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "grafatui-{name}-{}-{suffix}.toml",
            std::process::id()
        ))
    }

    #[test]
    fn imported_layout_replaces_the_initial_flat_layout() {
        let mut state = app::AppState::new(
            prom::PromClient::new("http://localhost:9090".to_string()),
            Duration::from_secs(60),
            Duration::from_secs(5),
            Duration::from_secs(1),
            "test".to_string(),
            app::default_queries(vec!["up".to_string()]),
            0,
            Theme::default(),
            "dashed-line".to_string(),
            export::ExportOptions::default(),
        );
        let layout = DashboardLayout::new(vec![DashboardLayoutItem::Row(DashboardRow::new(
            RowId::new(0),
            "Collapsed",
            true,
            false,
            vec![DashboardLayoutItem::Panel(0)],
        ))]);

        apply_imported_layout(&mut state, Some(layout));

        assert_eq!(state.visible_panel_indices(), Vec::<usize>::new());
    }

    #[test]
    fn startup_config_loader_propagates_parse_errors() {
        let path = temp_config_path("malformed-startup-config");
        std::fs::write(
            &path,
            "[annotations_command]\nprogram = \"./provider\"\ntimeout = \"soon\"\n",
        )
        .unwrap();

        let result = load_startup_config(Some(path.clone()));

        std::fs::remove_file(path).unwrap();
        assert!(result.is_err());
    }

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
    fn annotation_source_cli_command_replaces_complete_toml_source() {
        let resolved = resolve_annotation_source(
            AnnotationCliSource {
                program: Some("./cli-provider".into()),
                args: vec!["cli".into()],
                timeout: Some("2s".into()),
                ..AnnotationCliSource::default()
            },
            Some("config.jsonl".into()),
            None,
        )
        .unwrap();

        assert_eq!(
            resolved,
            Some(annotations::AnnotationSourceConfig::Command(
                annotations::AnnotationCommandConfig {
                    program: "./cli-provider".into(),
                    args: vec!["cli".into()],
                    timeout: Duration::from_secs(2),
                },
            ))
        );
    }

    #[test]
    fn annotation_source_rejects_invalid_same_layer_configuration() {
        let toml_command = annotations::AnnotationCommandConfig {
            program: "./config-provider".into(),
            args: vec![],
            timeout: Duration::from_secs(10),
        };
        assert!(
            resolve_annotation_source(
                AnnotationCliSource::default(),
                Some("events.jsonl".into()),
                Some(toml_command),
            )
            .is_err()
        );
        assert!(
            resolve_annotation_source(
                AnnotationCliSource {
                    program: Some("   ".into()),
                    ..AnnotationCliSource::default()
                },
                None,
                None,
            )
            .is_err()
        );
        assert!(
            resolve_annotation_source(
                AnnotationCliSource {
                    program: Some("./provider".into()),
                    timeout: Some("0s".into()),
                    ..AnnotationCliSource::default()
                },
                None,
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn annotation_source_returns_none_when_unconfigured() {
        assert_eq!(
            resolve_annotation_source(AnnotationCliSource::default(), None, None).unwrap(),
            None
        );
    }

    #[test]
    fn annotation_source_cli_file_replaces_toml_command() {
        let resolved = resolve_annotation_source(
            AnnotationCliSource {
                file: Some("cli.jsonl".into()),
                ..AnnotationCliSource::default()
            },
            None,
            Some(annotations::AnnotationCommandConfig {
                program: "./config-provider".into(),
                args: vec!["config".into()],
                timeout: Duration::from_secs(10),
            }),
        )
        .unwrap();

        assert_eq!(
            resolved,
            Some(annotations::AnnotationSourceConfig::File(
                "cli.jsonl".into()
            ))
        );
    }

    #[test]
    fn annotation_source_uses_toml_command_without_cli_source() {
        let command = annotations::AnnotationCommandConfig {
            program: "./config-provider".into(),
            args: vec!["config".into()],
            timeout: Duration::from_secs(3),
        };

        assert_eq!(
            resolve_annotation_source(AnnotationCliSource::default(), None, Some(command.clone()))
                .unwrap(),
            Some(annotations::AnnotationSourceConfig::Command(command))
        );
    }

    #[test]
    fn annotation_source_rejects_malformed_cli_timeout() {
        assert!(
            resolve_annotation_source(
                AnnotationCliSource {
                    program: Some("./provider".into()),
                    timeout: Some("soon".into()),
                    ..AnnotationCliSource::default()
                },
                None,
                None,
            )
            .is_err()
        );
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
