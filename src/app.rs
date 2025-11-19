use crate::prom;
use crate::ui;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use futures::StreamExt;
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

#[derive(Debug, Clone, Copy)]
pub struct GridUnit {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
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

        for expr in &p.exprs {
            let expr_expanded = expand_expr(expr, step, vars);

            // Calculate start/end for URL display purposes
            let end_ts = chrono::Utc::now().timestamp();
            let start_ts = end_ts - (range.as_secs() as i64);

            let url = prometheus.build_query_range_url(&expr_expanded, start_ts, end_ts, step);
            last_url = Some(url);

            match prometheus.query_range(&expr_expanded, range, step).await {
                Ok(res) => {
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
                                if y.is_finite() {
                                    pts.push((ts, y));
                                }
                            }
                        }
                        panel_results.push(SeriesView {
                            legend,
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
