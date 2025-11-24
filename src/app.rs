use crate::prom;
use crate::theme::Theme;
use crate::ui;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
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
    /// Y-axis scaling mode.
    pub y_axis_mode: YAxisMode,
    /// Visualization type.
    pub panel_type: PanelType,
}

/// Visualization types supported by Grafatui.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PanelType {
    Graph,
    Gauge,
    BarGauge,
    Table,
    Stat,
    Unknown,
}

/// Modes for Y-axis scaling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum YAxisMode {
    /// Auto-scale based on min/max of data.
    Auto,
    /// Always include zero.
    ZeroBased,
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
    /// Whether the series is visible in the chart.
    pub visible: bool,
}

/// Grid positioning unit (Grafana style).
#[derive(Debug, Clone, Copy)]
pub struct GridUnit {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Application mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode {
    Normal,
    Search,
    Fullscreen,
    Inspect,
    FullscreenInspect,
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
    /// Index of the currently selected panel.
    pub selected_panel: usize,
    /// UI Theme.
    pub theme: Theme,
    /// Time offset from "now" for panning backward in time (0 = live mode).
    pub time_offset: Duration,
    /// Current application mode.
    pub mode: AppMode,
    /// Search query string.
    pub search_query: String,
    /// Filtered panel indices based on search query.
    pub search_results: Vec<usize>,
    /// Cursor X position (timestamp) for inspection.
    pub cursor_x: Option<f64>,
}

impl AppState {
    /// Creates a new application state.
    ///
    /// # Arguments
    ///
    /// * `prometheus` - The Prometheus client.
    /// * `range` - The initial time range window.
    /// * `step` - The query resolution step.
    /// * `refresh_every` - The data refresh interval.
    /// * `title` - The dashboard title.
    /// * `panels` - The list of panels to display.
    /// * `skipped_panels` - The count of panels that were skipped during import.
    /// * `theme` - The UI theme to use.
    pub fn new(
        prometheus: prom::PromClient,
        range: Duration,
        step: Duration,
        refresh_every: Duration,
        title: String,
        panels: Vec<PanelState>,
        skipped_panels: usize,
        theme: Theme,
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
            selected_panel: 0,
            theme,
            time_offset: Duration::from_secs(0),
            mode: AppMode::Normal,
            search_query: String::new(),
            search_results: Vec::new(),
            cursor_x: None,
        }
    }

    /// Zoom in: halve the time range.
    pub fn zoom_in(&mut self) {
        self.range = self.range / 2;
        if self.range < Duration::from_secs(10) {
            self.range = Duration::from_secs(10);
        }
    }

    /// Zoom out: double the time range.
    pub fn zoom_out(&mut self) {
        self.range = self.range * 2;
        // Cap at 7 days
        if self.range > Duration::from_secs(7 * 24 * 3600) {
            self.range = Duration::from_secs(7 * 24 * 3600);
        }
    }

    /// Pan left: shift the time window backward.
    pub fn pan_left(&mut self) {
        // Shift by 25% of the current range
        let shift = self.range / 4;
        self.time_offset = self.time_offset.saturating_add(shift);
    }

    /// Pan right: shift the time window forward (toward "now").
    pub fn pan_right(&mut self) {
        // Shift by 25% of the current range
        let shift = self.range / 4;
        if self.time_offset > shift {
            self.time_offset = self.time_offset.saturating_sub(shift);
        } else {
            self.time_offset = Duration::from_secs(0); // Back to live mode
        }
    }

    /// Reset to live mode (time_offset = 0).
    pub fn reset_to_live(&mut self) {
        self.time_offset = Duration::from_secs(0);
    }

    /// Check if currently in live mode.
    pub fn is_live(&self) -> bool {
        self.time_offset.as_secs() == 0
    }

    /// Move cursor left/right by one step.
    pub fn move_cursor(&mut self, direction: i32) {
        if let Some(current_x) = self.cursor_x {
            let step_secs = self.step.as_secs_f64();
            let new_x = current_x + (direction as f64 * step_secs);

            // Clamp to current view range
            let end_ts =
                (chrono::Utc::now().timestamp() - self.time_offset.as_secs() as i64) as f64;
            let start_ts = end_ts - self.range.as_secs_f64();

            if new_x >= start_ts && new_x <= end_ts {
                self.cursor_x = Some(new_x);
            }
        } else {
            // Initialize cursor at center of view if not set
            let end_ts =
                (chrono::Utc::now().timestamp() - self.time_offset.as_secs() as i64) as f64;
            let start_ts = end_ts - self.range.as_secs_f64();
            self.cursor_x = Some((start_ts + end_ts) / 2.0);
        }
    }

    pub async fn refresh(&mut self) -> Result<()> {
        let prometheus = &self.prometheus;
        let range = self.range;
        let step = self.step;
        let vars = &self.vars;

        // Calculate end timestamp: "now" minus time_offset
        let end_ts = chrono::Utc::now().timestamp() - self.time_offset.as_secs() as i64;

        // Create a stream of futures for fetching panel data
        let mut futures = futures::stream::iter(self.panels.iter_mut())
            .map(|p| Self::fetch_single_panel_data(prometheus, p, range, step, vars, end_ts))
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
        end_ts: i64,
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
            let start_ts = end_ts - (range.as_secs() as i64);

            let url = prometheus.build_query_range_url(&expr_expanded, start_ts, end_ts, step);
            last_url = Some(url);

            match prometheus
                .query_range(&expr_expanded, start_ts, end_ts, step)
                .await
            {
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
                            visible: true,
                        });
                        // Downsample for display
                        if let Some(last) = panel_results.last_mut() {
                            last.points = downsample(last.points.clone(), 200);
                        }
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

/// Downsamples data points to a maximum number of points using max-pooling.
/// This preserves peaks which is important for metrics.
fn downsample(points: Vec<(f64, f64)>, max_points: usize) -> Vec<(f64, f64)> {
    if points.len() <= max_points {
        return points;
    }

    let chunk_size = (points.len() as f64 / max_points as f64).ceil() as usize;
    if chunk_size <= 1 {
        return points;
    }

    points
        .chunks(chunk_size)
        .filter_map(|chunk| {
            // Max pooling: take the point with the maximum value in the chunk
            chunk
                .iter()
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .cloned()
        })
        .collect()
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

    #[test]
    fn test_downsample() {
        let points: Vec<(f64, f64)> = (0..1000).map(|i| (i as f64, i as f64)).collect();
        let downsampled = downsample(points, 100);
        assert_eq!(downsampled.len(), 100);
        // Max pooling should preserve the max value in each chunk
        // Last point should be 999.0
        assert_eq!(downsampled.last().unwrap().1, 999.0);
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
            y_axis_mode: YAxisMode::Auto,
            panel_type: PanelType::Graph,
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
            match event::read()? {
                Event::Key(key) => {
                    if app.mode == AppMode::Search {
                        match key.code {
                            KeyCode::Esc => {
                                app.mode = AppMode::Normal;
                                app.search_query.clear();
                                app.search_results.clear();
                            }
                            KeyCode::Enter => {
                                if let Some(&idx) = app.search_results.first() {
                                    app.selected_panel = idx;
                                    app.mode = AppMode::Fullscreen; // Go to Fullscreen on selection
                                    app.search_query.clear();
                                    app.search_results.clear();
                                }
                            }
                            KeyCode::Backspace => {
                                app.search_query.pop();
                                // Update results
                                if app.search_query.is_empty() {
                                    app.search_results.clear();
                                } else {
                                    app.search_results = app
                                        .panels
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, p)| {
                                            p.title
                                                .to_lowercase()
                                                .contains(&app.search_query.to_lowercase())
                                        })
                                        .map(|(i, _)| i)
                                        .collect();
                                }
                            }
                            KeyCode::Char(c) => {
                                app.search_query.push(c);
                                // Update results
                                app.search_results = app
                                    .panels
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, p)| {
                                        p.title
                                            .to_lowercase()
                                            .contains(&app.search_query.to_lowercase())
                                    })
                                    .map(|(i, _)| i)
                                    .collect();
                            }
                            _ => {}
                        }
                    } else if app.mode == AppMode::Inspect {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('v') => {
                                app.mode = AppMode::Normal;
                                app.cursor_x = None;
                            }
                            KeyCode::Left => {
                                app.move_cursor(-1);
                            }
                            KeyCode::Right => {
                                app.move_cursor(1);
                            }
                            KeyCode::Char('q') => return Ok(()),
                            _ => {}
                        }
                    } else if app.mode == AppMode::Fullscreen {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('f') | KeyCode::Enter => {
                                app.mode = AppMode::Normal;
                            }
                            KeyCode::Char('v') => {
                                app.mode = AppMode::FullscreenInspect;
                                // Initialize cursor
                                let end_ts = (chrono::Utc::now().timestamp()
                                    - app.time_offset.as_secs() as i64)
                                    as f64;
                                let start_ts = end_ts - app.range.as_secs_f64();
                                app.cursor_x = Some((start_ts + end_ts) / 2.0);
                            }
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('r') | KeyCode::Char('R') => {
                                app.refresh().await?;
                            }
                            // Allow some navigation/interaction in fullscreen too?
                            // For now, just basic ones.
                            KeyCode::Char('+') => {
                                app.zoom_out();
                                app.refresh().await?;
                            }
                            KeyCode::Char('-') => {
                                app.zoom_in();
                                app.refresh().await?;
                            }
                            KeyCode::Char('[') => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    app.pan_left();
                                    app.refresh().await?;
                                }
                            }
                            KeyCode::Left => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    app.pan_left();
                                    app.refresh().await?;
                                }
                            }
                            KeyCode::Char(']') => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    app.pan_right();
                                    app.refresh().await?;
                                }
                            }
                            KeyCode::Right => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    app.pan_right();
                                    app.refresh().await?;
                                }
                            }
                            KeyCode::Char('0') => {
                                app.reset_to_live();
                                app.refresh().await?;
                            }
                            KeyCode::Char('y') => {
                                if let Some(panel) = app.panels.get_mut(app.selected_panel) {
                                    panel.y_axis_mode = match panel.y_axis_mode {
                                        YAxisMode::Auto => YAxisMode::ZeroBased,
                                        YAxisMode::ZeroBased => YAxisMode::Auto,
                                    };
                                }
                            }
                            _ => {}
                        }
                    } else if app.mode == AppMode::FullscreenInspect {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('v') => {
                                app.mode = AppMode::Fullscreen;
                                app.cursor_x = None;
                            }
                            KeyCode::Left => {
                                app.move_cursor(-1);
                            }
                            KeyCode::Right => {
                                app.move_cursor(1);
                            }
                            KeyCode::Char('q') => return Ok(()),
                            _ => {}
                        }
                    } else {
                        // Normal Mode
                        match key.code {
                            KeyCode::Char('f') => {
                                app.mode = AppMode::Fullscreen;
                            }
                            KeyCode::Char('v') => {
                                app.mode = AppMode::Inspect;
                                // Initialize cursor
                                let end_ts = (chrono::Utc::now().timestamp()
                                    - app.time_offset.as_secs() as i64)
                                    as f64;
                                let start_ts = end_ts - app.range.as_secs_f64();
                                app.cursor_x = Some((start_ts + end_ts) / 2.0);
                            }
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('r') | KeyCode::Char('R') => {
                                app.refresh().await?;
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.selected_panel > 0 {
                                    app.selected_panel -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.selected_panel < app.panels.len().saturating_sub(1) {
                                    app.selected_panel += 1;
                                }
                            }
                            KeyCode::PageUp => {
                                app.vertical_scroll = app.vertical_scroll.saturating_sub(10);
                            }
                            KeyCode::PageDown => {
                                app.vertical_scroll = app.vertical_scroll.saturating_add(10);
                            }
                            KeyCode::Char(c) if c.is_digit(10) => {
                                if let Some(digit) = c.to_digit(10) {
                                    if let Some(panel) = app.panels.get_mut(app.selected_panel) {
                                        if digit == 0 {
                                            // Show all
                                            for s in &mut panel.series {
                                                s.visible = true;
                                            }
                                        } else {
                                            // Toggle specific series (1-based index)
                                            let idx = (digit - 1) as usize;
                                            if let Some(series) = panel.series.get_mut(idx) {
                                                series.visible = !series.visible;
                                            }
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('y') => {
                                if let Some(panel) = app.panels.get_mut(app.selected_panel) {
                                    panel.y_axis_mode = match panel.y_axis_mode {
                                        YAxisMode::Auto => YAxisMode::ZeroBased,
                                        YAxisMode::ZeroBased => YAxisMode::Auto,
                                    };
                                }
                            }
                            KeyCode::Home => {
                                app.vertical_scroll = 0;
                            }
                            KeyCode::End => {
                                app.vertical_scroll = usize::MAX; // Will be clamped by rendering logic usually, or we should track max height
                            }
                            KeyCode::Char('+') => {
                                app.zoom_out();
                                app.refresh().await?;
                            }
                            KeyCode::Char('-') => {
                                app.zoom_in();
                                app.refresh().await?;
                            }
                            KeyCode::Char('[') => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    app.pan_left();
                                    app.refresh().await?;
                                }
                            }
                            KeyCode::Left => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    app.pan_left();
                                    app.refresh().await?;
                                }
                            }
                            KeyCode::Char(']') => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    app.pan_right();
                                    app.refresh().await?;
                                }
                            }
                            KeyCode::Right => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    app.pan_right();
                                    app.refresh().await?;
                                }
                            }
                            KeyCode::Char('0') => {
                                app.reset_to_live();
                                app.refresh().await?;
                            }
                            KeyCode::Char('?') => {
                                app.debug_bar = !app.debug_bar;
                            }
                            KeyCode::Char('/') => {
                                app.mode = AppMode::Search;
                                app.search_query.clear();
                                app.search_results.clear();
                            }
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
                    | crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) =>
                    {
                        let size = terminal.size()?;
                        let rect = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                        if let Some((idx, panel_rect)) =
                            ui::hit_test(app, rect, mouse.column, mouse.row)
                        {
                            app.selected_panel = idx;

                            // If in Fullscreen or FullscreenInspect, we are already focused on this panel (effectively)
                            // If in Normal/Inspect, we switch to Inspect mode if not already

                            match app.mode {
                                AppMode::Normal | AppMode::Inspect => {
                                    app.mode = AppMode::Inspect;
                                }
                                AppMode::Fullscreen | AppMode::FullscreenInspect => {
                                    app.mode = AppMode::FullscreenInspect;
                                }
                                _ => {}
                            }

                            // Calculate cursor_x based on click position within panel_rect
                            // Chart area is inside the block borders, so we need to account for that.
                            // Assuming borders are 1 char wide.
                            let chart_width = panel_rect.width.saturating_sub(2) as f64;
                            if chart_width > 0.0 {
                                let relative_x =
                                    (mouse.column.saturating_sub(panel_rect.x + 1)) as f64;
                                let fraction = (relative_x / chart_width).clamp(0.0, 1.0);

                                let end_ts = (chrono::Utc::now().timestamp()
                                    - app.time_offset.as_secs() as i64)
                                    as f64;
                                let start_ts = end_ts - app.range.as_secs_f64();

                                app.cursor_x = Some(start_ts + fraction * app.range.as_secs_f64());
                            }
                        }
                    }
                    crossterm::event::MouseEventKind::ScrollDown => {
                        app.vertical_scroll = app.vertical_scroll.saturating_add(1);
                    }
                    crossterm::event::MouseEventKind::ScrollUp => {
                        app.vertical_scroll = app.vertical_scroll.saturating_sub(1);
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        if should_refresh {
            app.refresh().await?;
        }

        sleep(Duration::from_millis(10)).await;
    }
}
