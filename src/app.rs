use crate::prom;
use crate::ui;
use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode};
use ratatui::Terminal;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct PanelState {
    pub title: String,
    pub exprs: Vec<String>,
    pub series: Vec<SeriesView>,
    pub last_error: Option<String>,
    pub last_url: Option<String>,
    pub last_samples: usize,
    pub grid: Option<GridUnit>,
}

#[derive(Debug, Clone)]
pub struct SeriesView {
    pub legend: String,
    pub points: Vec<(f64, f64)>,
}

// add near the top (after use statements)
#[derive(Debug, Clone, Copy)]
pub struct GridUnit {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

fn build_query_range_url(base: &str, expr: &str, range: Duration, step: Duration) -> String {
    use chrono::Utc;
    use std::cmp::max;
    let end = Utc::now().timestamp();
    let start = end - (range.as_secs() as i64);
    let step_s = max(1, step.as_secs());
    let step_param = format!("{}s", step_s);
    format!(
        "{}/api/v1/query_range?query={}&start={}&end={}&step={}",
        base.trim_end_matches('/'),
        urlencoding::encode(expr),
        start,
        end,
        step_param
    )
}

#[derive(Debug)]
pub struct AppState {
    pub prometheus: prom::PromClient,
    pub range: Duration,
    pub step: Duration,
    pub refresh_every: Duration,
    pub panels: Vec<PanelState>,
    pub last_refresh: Instant,
    pub vertical_scroll: usize,
    pub title: String,
    pub debug_bar: bool,
    pub vars: HashMap<String, String>,
}

impl AppState {
    pub fn new(
        prometheus: prom::PromClient,
        range: Duration,
        step: Duration,
        refresh_every: Duration,
        title: String,
        panels: Vec<PanelState>,
    ) -> Self {
        Self {
            prometheus,
            range,
            step,
            refresh_every,
            panels,
            last_refresh: Instant::now() - refresh_every,
            vertical_scroll: 0,
            title,
            debug_bar: false,
            vars: HashMap::new(),
        }
    }

    pub async fn refresh(&mut self) -> Result<()> {
        let prometheus = &self.prometheus;
        let range = self.range;
        let step = self.step;

        for p in &mut self.panels {
            match Self::fetch_panel_static(prometheus, p, range, step, &self.vars).await {
                Ok(()) => p.last_error = None,
                Err(e) => p.last_error = Some(format!("{}", e)),
            }
        }
        self.last_refresh = Instant::now();
        Ok(())
    }

    async fn fetch_panel_static(
        prometheus: &prom::PromClient,
        p: &mut PanelState,
        range: Duration,
        step: Duration,
        vars: &HashMap<String, String>,
    ) -> Result<()> {
        let mut all_series = Vec::new();
        for expr in &p.exprs {
            let expr_expanded = expand_expr(expr, step, vars);
            let url = build_query_range_url(&prometheus.base, &expr_expanded, range, step);
            p.last_url = Some(url);
            let res = prometheus
                .query_range(&expr_expanded, range, step)
                .await
                .with_context(|| format!("query_range failed for `{}`", expr_expanded))?;
            for s in res {
                let legend = if s.metric.is_empty() {
                    expr_expanded.clone()
                } else {
                    let mut labels: Vec<_> = s
                        .metric
                        .iter()
                        .map(|(k, v)| format!("{}=\"{}\"", k, v))
                        .collect();
                    labels.sort();
                    format!("{} {{{}}}", expr_expanded, labels.join(", "))
                };
                let mut pts = Vec::with_capacity(s.values.len());
                for (ts, val) in s.values {
                    if let Ok(y) = val.parse::<f64>() {
                        pts.push((ts, y));
                    }
                }
                all_series.push(SeriesView {
                    legend,
                    points: pts,
                });
            }
        }
        let samples: usize = all_series.iter().map(|s| s.points.len()).sum();
        p.last_samples = samples;
        p.series = all_series;
        Ok(())
    }
}

fn expand_expr(expr: &str, step: Duration, vars: &HashMap<String, String>) -> String {
    let mut s = expr.to_string();
    // 1) $__rate_interval -> step seconds (simple approximation)
    let step_param = format!("{}s", step.as_secs().max(1));
    s = s.replace("$__rate_interval", &step_param);
    // 2) ${var} and $var -> value from vars
    for (k, v) in vars {
        s = s.replace(&format!("${{{}}}", k), v);
        s = s.replace(&format!("${}", k), v);
    }
    s
}

pub fn default_queries(mut provided: Vec<String>) -> Vec<PanelState> {
    if provided.is_empty() {
        provided = vec![
            r#"sum(rate(http_requests_total{job!="prometheus"}[5m]))"#.to_string(),
            r#"sum by (instance) (process_cpu_seconds_total)"#.to_string(),
            r#"up"#.to_string(),
        ];
    }
    provided
        .into_iter()
        .map(|q| PanelState {
            title: q.clone(),
            exprs: vec![q],
            series: vec![],
            last_error: None,
            last_url: None,
            last_samples: 0,
            grid: None,
        })
        .collect()
}

pub fn parse_duration(s: &str) -> Result<Duration> {
    Ok(humantime::parse_duration(s)?)
}

pub async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut AppState,
    tick_rate: Duration,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw_ui(f, app))?;

        let timeout = tick_rate.saturating_sub(app.last_refresh.elapsed().min(tick_rate));
        let should_refresh = app.last_refresh.elapsed() >= app.refresh_every;

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        app.refresh().await?;
                    }
                    KeyCode::Up => {
                        app.vertical_scroll = app.vertical_scroll.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        app.vertical_scroll = app.vertical_scroll.saturating_add(1);
                    }
                    KeyCode::Char('+') => {
                        app.range = app.range.saturating_mul(2);
                        app.refresh().await?;
                    }
                    KeyCode::Char('-') => {
                        app.range = app.range / 2;
                        if app.range < Duration::from_secs(10) {
                            app.range = Duration::from_secs(10);
                        }
                        app.refresh().await?;
                    }
                    KeyCode::Char('?') => {
                        app.debug_bar = !app.debug_bar;
                    }
                    _ => {}
                }
            }
        }

        if should_refresh {
            app.refresh().await?;
        }

        sleep(Duration::from_millis(10)).await;
    }
}
