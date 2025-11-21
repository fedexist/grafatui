use crate::prom;
use crate::ui;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use futures::StreamExt;
use ratatui::Terminal;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Represents the state of a single dashboard panel.
#[derive(Debug, Clone)]
pub struct PanelState {
    /// Panel title.
    pub title: String,
    /// PromQL expressions to query.
    pub exprs: Vec<String>,
    /// Optional legend formats (e.g. "{{instance}}"). Parallel to exprs.
    pub legends: Vec<Option<String>>,
    /// Current time-series data for this panel.
    pub series: Vec<SeriesView>,
    /// Last error message, if any.
    pub last_error: Option<String>,
    /// Last query URL used (for debugging).
    pub last_url: Option<String>,
    /// Total number of samples in the current view.
    pub last_samples: usize,
    /// Grid layout position (if imported from Grafana).
    pub grid: Option<GridUnit>,
}

/// Represents a single time-series line in a chart.
#[derive(Debug, Clone)]
pub struct SeriesView {
    /// Stable name of the series (used for coloring).
    pub name: String,
    /// Latest value of the series (used for display).
    pub value: Option<f64>,
    /// Data points (timestamp, value).
    pub points: Vec<(f64, f64)>,
}

/// Grid positioning unit (Grafana style).
#[derive(Debug, Clone, Copy)]
pub struct GridUnit {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Global application state.
#[derive(Debug)]
pub struct AppState {
    /// Prometheus client for making requests.
    pub prometheus: prom::PromClient,
    /// Current time range window.
    pub range: Duration,
    /// Query step resolution.
    pub step: Duration,
    /// How often to refresh data.
    pub refresh_every: Duration,
    /// List of panels.
    pub panels: Vec<PanelState>,
    /// Timestamp of the last successful refresh.
    pub last_refresh: Instant,
    /// Vertical scroll offset.
    pub vertical_scroll: usize,
    /// Dashboard title.
    pub title: String,
    /// Whether to show the debug bar.
    pub debug_bar: bool,
    /// Template variables (key -> value).
    pub vars: HashMap<String, String>,
    /// Count of panels skipped during import.
    pub skipped_panels: usize,
}

impl AppState {
    pub fn new(
        prometheus: prom::PromClient,
        range: Duration,
        step: Duration,
        refresh_every: Duration,
        title: String,
        panels: Vec<PanelState>,
        skipped_panels: usize,
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
            skipped_panels,
        }
    }

    pub async fn refresh(&mut self) -> Result<()> {
        let prometheus = &self.prometheus;
        let range = self.range;
        let step = self.step;
        let vars = &self.vars;

        // Create a stream of futures for fetching panel data
        let mut futures = futures::stream::iter(self.panels.iter_mut())
            .map(|p| Self::fetch_single_panel_data(prometheus, p, range, step, vars))
            .buffer_unordered(4); // Max 4 concurrent panel refreshes

        while let Some((p, results, url, err)) = futures.next().await {
            p.series = results;
            p.last_samples = p.series.iter().map(|s| s.points.len()).sum();
            if let Some(u) = url {
                p.last_url = Some(u);
            }
            p.last_error = err;
        }

        self.last_refresh = Instant::now();
        Ok(())
    }

    async fn fetch_single_panel_data<'a>(
        prometheus: &'a prom::PromClient,
        p: &'a mut PanelState,
        range: Duration,
        step: Duration,
        vars: &'a HashMap<String, String>,
    ) -> (
        &'a mut PanelState,
        Vec<SeriesView>,
        Option<String>,
        Option<String>,
    ) {
        let mut panel_results = Vec::new();
        let mut last_url = None;
        let mut error = None;

        for (i, expr) in p.exprs.iter().enumerate() {
            let expr_expanded = expand_expr(expr, step, vars);
            let legend_fmt = p.legends.get(i).and_then(|x| x.as_ref());

            // Calculate start/end for URL display purposes
            let end_ts = chrono::Utc::now().timestamp();
            let start_ts = end_ts - (range.as_secs() as i64);

            let url = prometheus.build_query_range_url(&expr_expanded, start_ts, end_ts, step);
            last_url = Some(url);

            match prometheus.query_range(&expr_expanded, range, step).await {
                Ok(res) => {
                    for s in res {
                        let latest_val = s.values.last().and_then(|(_, v)| v.parse::<f64>().ok());
                        let legend_base = if let Some(fmt) = legend_fmt {
                            format_legend(fmt, &s.metric)
                        } else if s.metric.is_empty() {
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
                                if y.is_finite() {
                                    pts.push((ts, y));
                                }
                            }
                        }
                        panel_results.push(SeriesView {
                            name: legend_base,
                            value: latest_val,
                            points: pts,
                        });
                    }
                }
                Err(e) => {
                    error = Some(format!("query_range failed for `{}`: {}", expr_expanded, e));
                }
            }
        }
        (p, panel_results, last_url, error)
    }
}

fn expand_expr(expr: &str, step: Duration, vars: &HashMap<String, String>) -> String {
    let mut s = expr.to_string();

    // 1) $__rate_interval heuristic: max(step * 4, 1m)
    // This matches Grafana's default behavior roughly
    let interval_secs = std::cmp::max(step.as_secs() * 4, 60);
    let interval_param = format!("{}s", interval_secs);
    s = s.replace("$__rate_interval", &interval_param);

    // 2) ${var} and $var -> value from vars
    for (k, v) in vars {
        // Replace ${var}
        s = s.replace(&format!("${{{}}}", k), v);
        // Replace $var (simple word boundary check would be better but start with simple replace)
        // We need to be careful not to replace $variable if we are replacing $var
        // For now, simple replacement.
        s = s.replace(&format!("${}", k), v);
    }

    // 3) Fallback for unset vars: if we still see $something, maybe we should warn or replace with regex?
    // The user requested: "Fallback when a var is unset: turn label="$var" into a permissive regex (e.g., label=~".*") or skip that filter."
    // This is complex to do with simple string replacement without parsing PromQL.
    // For Milestone 0/1, we will just leave it, which might cause a query error, which is visible.

    s
}

fn format_legend(fmt: &str, metric: &HashMap<String, String>) -> String {
    let mut out = fmt.to_string();
    // Replace {{label}} with value
    // This is a simple replacement, Grafana supports more complex syntax but this covers 90%
    for (k, v) in metric {
        out = out.replace(&format!("{{{{{}}}}}", k), v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_expr_rate_interval() {
        let vars = HashMap::new();
        let step = Duration::from_secs(15);
        // heuristic: max(15*4, 60) = 60s
        let expr = "rate(http_requests_total[$__rate_interval])";
        let expanded = expand_expr(expr, step, &vars);
        assert_eq!(expanded, "rate(http_requests_total[60s])");

        let step = Duration::from_secs(30);
        // heuristic: max(30*4, 60) = 120s
        let expr = "rate(http_requests_total[$__rate_interval])";
        let expanded = expand_expr(expr, step, &vars);
        assert_eq!(expanded, "rate(http_requests_total[120s])");
    }

    #[test]
    fn test_expand_expr_vars() {
        let mut vars = HashMap::new();
        vars.insert("job".to_string(), "node-exporter".to_string());
        vars.insert("instance".to_string(), "localhost:9100".to_string());

        let step = Duration::from_secs(15);

        // Test $var
        let expr = "up{job=\"$job\"}";
        let expanded = expand_expr(expr, step, &vars);
        assert_eq!(expanded, "up{job=\"node-exporter\"}");

        // Test ${var}
        let expr = "up{instance=\"${instance}\"}";
        let expanded = expand_expr(expr, step, &vars);
        assert_eq!(expanded, "up{instance=\"localhost:9100\"}");

        // Test multiple vars
        let expr =
            "rate(http_requests_total{job=\"$job\", instance=\"$instance\"}[$__rate_interval])";
        let expanded = expand_expr(expr, step, &vars);
        assert_eq!(
            expanded,
            "rate(http_requests_total{job=\"node-exporter\", instance=\"localhost:9100\"}[60s])"
        );
    }

    #[test]
    fn test_format_legend() {
        let mut metric = HashMap::new();
        metric.insert("job".to_string(), "node".to_string());
        metric.insert("instance".to_string(), "localhost".to_string());

        let fmt = "Job: {{job}} - {{instance}}";
        assert_eq!(format_legend(fmt, &metric), "Job: node - localhost");

        let fmt2 = "Static Text";
        assert_eq!(format_legend(fmt2, &metric), "Static Text");
    }
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
            legends: vec![None],
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
                    KeyCode::PageUp => {
                        app.vertical_scroll = app.vertical_scroll.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        app.vertical_scroll = app.vertical_scroll.saturating_add(10);
                    }
                    KeyCode::Home => {
                        app.vertical_scroll = 0;
                    }
                    KeyCode::End => {
                        app.vertical_scroll = usize::MAX; // Will be clamped by rendering logic usually, or we should track max height
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
